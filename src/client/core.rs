use std::{num::NonZeroU32, time::Duration};

use rustopus::{Arm, Backoff, Quota};

use crate::decode::{decode_code, decode_layer, decode_metadata};
use crate::error::BojError;
use crate::model::{CodeResponse, LayerResponse, MetadataResponse};
use crate::query::{CodeQuery, LayerQuery, MetadataQuery};
use crate::reqwest;

use super::http::{build_url, header_value};
use super::policy::{BojPolicy, map_arm_error};
use super::resilience::BojResilienceConfig;
use super::response::{ensure_success_status, normalize_response_body};

/// Production BOJ endpoint origin.
pub const BOJ_BASE_URL: &str = "https://www.stat-search.boj.or.jp";

/// Async BOJ API client with retries and shared rate/concurrency limits.
///
/// Clones share one Rustopus arm. Construct one client per API and clone it
/// for concurrent tasks so they all respect the same limits. Methods require
/// a Tokio runtime; construction does not create or own a runtime.
///
/// ```no_run
/// use boj_client::{BojClient, query::{CodeQuery, Format, Language}};
/// # async fn example() -> Result<(), boj_client::error::BojError> {
/// let client = BojClient::new()?;
/// let query = CodeQuery::new("CO", vec!["TK99F1000601GCQ01000".into()])?
///     .with_format(Format::Json).with_lang(Language::En);
/// let series = client.get_data_code_all(&query).await?;
/// # Ok(()) }
/// ```
#[derive(Clone)]
pub struct BojClient {
    http: reqwest::Client,
    arm: Arm<BojPolicy>,
    base_url: String,
    request_timeout: Duration,
    pub(super) max_pages: usize,
}

impl BojClient {
    /// Creates a client with the default resilience configuration.
    ///
    /// # Errors
    /// Returns an error if the HTTP client cannot be built.
    pub fn new() -> Result<Self, BojError> {
        Self::with_config(BojResilienceConfig::default())
    }

    /// Creates a default HTTP client with custom resilience settings.
    ///
    /// # Errors
    /// Returns an error for invalid settings or HTTP client construction failure.
    pub fn with_config(config: BojResilienceConfig) -> Result<Self, BojError> {
        config.validate()?;
        let http = reqwest::Client::builder()
            .user_agent(format!("boj-client/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| BojError::transport(error.to_string()))?;
        Self::with_client(http, BOJ_BASE_URL, config)
    }

    /// Creates a client using an injected async HTTP client and endpoint origin.
    ///
    /// Proxy and TLS settings come from `http`. Each attempt uses the configured
    /// request timeout. All endpoints and client clones share one arm.
    ///
    /// # Errors
    /// Returns an error for invalid settings or a base URL without an HTTP(S) host.
    pub fn with_client(
        http: reqwest::Client,
        base_url: impl Into<String>,
        config: BojResilienceConfig,
    ) -> Result<Self, BojError> {
        config.validate()?;
        let base_url = base_url.into();
        let url = reqwest::Url::parse(&base_url)
            .map_err(|error| BojError::validation(format!("invalid base URL: {error}")))?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(BojError::validation(
                "base URL must be HTTP(S) with a host and no query or fragment",
            ));
        }
        let mut builder = Arm::builder()
            .name("boj")
            .policy(BojPolicy {
                max_retry_after: config.max_retry_after,
            })
            .suckers(config.max_concurrent_requests)
            .max_attempts(config.max_attempts)
            .backoff(Backoff::exponential(config.initial_backoff).cap(config.max_backoff))
            .total_timeout(config.total_timeout);
        if let Some(rate) = NonZeroU32::new(config.requests_per_second) {
            builder = builder.rate(
                Quota::per_second(rate)
                    .allow_burst(NonZeroU32::new(config.burst_size).expect("validated burst")),
            );
        }
        Ok(Self {
            arm: builder.build(http.clone()),
            http,
            base_url,
            request_timeout: config.request_timeout,
            max_pages: config.max_pages,
        })
    }

    /// Injects an async HTTP client with default resilience settings.
    ///
    /// # Errors
    /// Returns an error if client configuration is invalid.
    pub fn with_reqwest_client(http: reqwest::Client) -> Result<Self, BojError> {
        Self::with_client(http, BOJ_BASE_URL, BojResilienceConfig::default())
    }

    /// Replaces the endpoint origin, primarily for mock servers.
    /// Invalid URLs are reported when a request is built.
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    async fn execute_request(
        &self,
        endpoint: &str,
        query_pairs: Vec<(String, String)>,
    ) -> Result<rustopus::Outcome, BojError> {
        let request = self
            .http
            .get(build_url(&self.base_url, endpoint, &query_pairs))
            .header("Accept-Encoding", "gzip")
            .timeout(self.request_timeout)
            .build()
            .map_err(|error| BojError::TransportError {
                kind: rustopus::TransportErrorKind::Request,
                message: error.to_string(),
            })?;
        self.arm.send(request).await.map_err(map_arm_error)
    }

    /// Calls `getDataCode` and decodes the response into [`CodeResponse`].
    ///
    /// # Errors
    ///
    /// Returns [`BojError`] when request sending fails, response decoding fails,
    /// or when BOJ returns `STATUS != 200`.
    pub async fn get_data_code(&self, query: &CodeQuery) -> Result<CodeResponse, BojError> {
        let response = self
            .execute_request(query.endpoint(), query.query_pairs())
            .await?;

        let content_type = header_value(&response, "content-type");
        let body = normalize_response_body(&response)?;
        let decoded = decode_code(&body, content_type.as_deref(), query.csv_encoding_hint())?;

        ensure_success_status(
            decoded.meta.status,
            &decoded.meta.message_id,
            &decoded.meta.message,
        )?;

        Ok(decoded)
    }

    /// Calls `getDataLayer` and decodes the response into [`LayerResponse`].
    ///
    /// # Errors
    ///
    /// Returns [`BojError`] when request sending fails, response decoding fails,
    /// or when BOJ returns `STATUS != 200`.
    pub async fn get_data_layer(&self, query: &LayerQuery) -> Result<LayerResponse, BojError> {
        let response = self
            .execute_request(query.endpoint(), query.query_pairs())
            .await?;

        let content_type = header_value(&response, "content-type");
        let body = normalize_response_body(&response)?;
        let decoded = decode_layer(&body, content_type.as_deref(), query.csv_encoding_hint())?;

        ensure_success_status(
            decoded.meta.status,
            &decoded.meta.message_id,
            &decoded.meta.message,
        )?;

        Ok(decoded)
    }

    /// Calls `getMetadata` and decodes the response into [`MetadataResponse`].
    ///
    /// # Errors
    ///
    /// Returns [`BojError`] when request sending fails, response decoding fails,
    /// or when BOJ returns `STATUS != 200`.
    pub async fn get_metadata(&self, query: &MetadataQuery) -> Result<MetadataResponse, BojError> {
        let response = self
            .execute_request(query.endpoint(), query.query_pairs())
            .await?;

        let content_type = header_value(&response, "content-type");
        let body = normalize_response_body(&response)?;
        let decoded = decode_metadata(&body, content_type.as_deref(), query.csv_encoding_hint())?;

        ensure_success_status(
            decoded.meta.status,
            &decoded.meta.message_id,
            &decoded.meta.message,
        )?;

        Ok(decoded)
    }
}
