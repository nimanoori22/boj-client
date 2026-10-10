use std::{
    io::Write,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use boj_client::{
    BojClient, BojResilienceConfig,
    error::BojError,
    query::{CodeQuery, Format},
};
use flate2::{Compression, write::GzEncoder};
use serde_json::json;
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate, matchers::path};

fn config() -> BojResilienceConfig {
    BojResilienceConfig {
        requests_per_second: 0,
        initial_backoff: Duration::from_millis(1),
        max_backoff: Duration::from_millis(1),
        ..Default::default()
    }
}

fn client(server: &MockServer, config: BojResilienceConfig) -> BojClient {
    BojClient::with_client(
        boj_client::reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap(),
        server.uri(),
        config,
    )
    .unwrap()
}

fn query() -> CodeQuery {
    CodeQuery::new("CO", vec!["A".into()])
        .unwrap()
        .with_format(Format::Json)
}

fn success() -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "STATUS": 200, "MESSAGEID": "M181000I", "MESSAGE": "OK", "RESULTSET": []
    }))
}

struct Sequence {
    count: Arc<AtomicUsize>,
    first: ResponseTemplate,
    then: ResponseTemplate,
}

impl Respond for Sequence {
    fn respond(&self, _: &Request) -> ResponseTemplate {
        if self.count.fetch_add(1, Ordering::SeqCst) == 0 {
            self.first.clone()
        } else {
            self.then.clone()
        }
    }
}

async fn retry_then_success(
    first: ResponseTemplate,
    config: BojResilienceConfig,
) -> (usize, Duration) {
    let server = MockServer::start().await;
    let count = Arc::new(AtomicUsize::new(0));
    Mock::given(path("/api/v1/getDataCode"))
        .respond_with(Sequence {
            count: count.clone(),
            first,
            then: success(),
        })
        .mount(&server)
        .await;
    let started = Instant::now();
    client(&server, config)
        .get_data_code(&query().with_format(Format::Csv))
        .await
        .unwrap();
    (count.load(Ordering::SeqCst), started.elapsed())
}

#[tokio::test]
async fn retries_transient_http_and_boj_statuses() {
    for status in [408, 429, 500, 502, 503, 504] {
        assert_eq!(
            retry_then_success(ResponseTemplate::new(status), config())
                .await
                .0,
            2
        );
    }
    for status in [500, 503] {
        let first = ResponseTemplate::new(200).set_body_json(json!({
            "STATUS": status, "MESSAGEID": "M181090S", "MESSAGE": "temporary"
        }));
        assert_eq!(retry_then_success(first, config()).await.0, 2);
    }
}

#[tokio::test]
async fn retries_gzip_json_error_for_csv_request() {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(br#"{"STATUS":503,"MESSAGEID":"M181091S","MESSAGE":"temporary"}"#)
        .unwrap();
    let first = ResponseTemplate::new(200)
        .insert_header("content-encoding", "gzip")
        .insert_header("content-type", "text/csv")
        .set_body_bytes(encoder.finish().unwrap());
    assert_eq!(retry_then_success(first, config()).await.0, 2);
}

#[tokio::test]
async fn retry_after_is_honoured_and_capped() {
    let mut settings = config();
    settings.max_retry_after = Duration::from_millis(40);
    let (count, elapsed) = retry_then_success(
        ResponseTemplate::new(429).insert_header("retry-after", "9999"),
        settings,
    )
    .await;
    assert_eq!(count, 2);
    assert!(elapsed >= Duration::from_millis(40));
    assert!(elapsed < Duration::from_secs(3));
}

#[tokio::test]
async fn permanent_errors_and_malformed_payloads_do_not_retry() {
    let cases = [
        (
            ResponseTemplate::new(200)
                .set_body_json(json!({"STATUS":400,"MESSAGEID":"bad","MESSAGE":"invalid"})),
            "api",
        ),
        (
            ResponseTemplate::new(404).set_body_string("missing"),
            "http",
        ),
        (
            ResponseTemplate::new(200).set_body_string("{broken json"),
            "decode",
        ),
        (
            ResponseTemplate::new(200)
                .insert_header("content-encoding", "gzip")
                .set_body_bytes(b"broken gzip"),
            "decode",
        ),
    ];
    for (response, expected) in cases {
        let server = MockServer::start().await;
        Mock::given(path("/api/v1/getDataCode"))
            .respond_with(response)
            .expect(1)
            .mount(&server)
            .await;
        let error = client(&server, config())
            .get_data_code(&query())
            .await
            .unwrap_err();
        assert!(
            matches!(
                (&error, expected),
                (BojError::ApiError { status: 400, .. }, "api")
                    | (BojError::HttpError { status: 404, .. }, "http")
                    | (BojError::DecodeError(_), "decode")
            ),
            "{error:?}"
        );
    }
}

#[tokio::test]
async fn exhaustion_preserves_final_response_and_attempt_count() {
    for http_status in [200, 503] {
        let server = MockServer::start().await;
        Mock::given(path("/api/v1/getDataCode"))
            .respond_with(ResponseTemplate::new(http_status).set_body_json(
                json!({"STATUS":503,"MESSAGEID":"M181091S","MESSAGE":"db unavailable"}),
            ))
            .expect(3)
            .mount(&server)
            .await;
        let mut settings = config();
        settings.max_attempts = 3;
        let error = client(&server, settings)
            .get_data_code(&query())
            .await
            .unwrap_err();
        assert!(
            matches!(error, BojError::RetryExhausted { attempts:3, last }
            if *last == BojError::api(503,"M181091S","db unavailable"))
        );
    }
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/getDataCode"))
        .respond_with(
            ResponseTemplate::new(503)
                .insert_header("x-detail", "last")
                .set_body_string("unavailable"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let mut settings = config();
    settings.max_attempts = 1;
    let error = client(&server, settings)
        .get_data_code(&query())
        .await
        .unwrap_err();
    assert!(
        matches!(error, BojError::RetryExhausted { attempts:1, last }
        if matches!(&*last, BojError::HttpError { status:503, headers, body }
            if headers.get("x-detail").map(String::as_str) == Some("last") && body.as_slice() == b"unavailable"))
    );
}

#[derive(Clone)]
struct TimedResponses(Arc<Mutex<Vec<Instant>>>);

impl Respond for TimedResponses {
    fn respond(&self, _: &Request) -> ResponseTemplate {
        self.0.lock().unwrap().push(Instant::now());
        success()
    }
}

#[tokio::test]
async fn clones_share_a_smooth_rate_limit() {
    let server = MockServer::start().await;
    let times = Arc::new(Mutex::new(Vec::new()));
    Mock::given(path("/api/v1/getDataCode"))
        .respond_with(TimedResponses(times.clone()))
        .expect(3)
        .mount(&server)
        .await;
    let mut settings = config();
    settings.requests_per_second = 10;
    settings.burst_size = 1;
    let first = client(&server, settings);
    let second = first.clone();
    let third = first.clone();
    let query = query();
    let (a, b, c) = tokio::join!(
        first.get_data_code(&query),
        second.get_data_code(&query),
        third.get_data_code(&query)
    );
    a.unwrap();
    b.unwrap();
    c.unwrap();
    let times = times.lock().unwrap();
    assert!(times[2].duration_since(times[0]) >= Duration::from_millis(180));
}

#[tokio::test]
async fn clones_share_concurrency_slots() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/getDataCode"))
        .respond_with(success().set_delay(Duration::from_millis(60)))
        .expect(3)
        .mount(&server)
        .await;
    let mut settings = config();
    settings.max_concurrent_requests = 1;
    let first = client(&server, settings);
    let second = first.clone();
    let third = first.clone();
    let query = query();
    let started = Instant::now();
    let (a, b, c) = tokio::join!(
        first.get_data_code(&query),
        second.get_data_code(&query),
        third.get_data_code(&query)
    );
    a.unwrap();
    b.unwrap();
    c.unwrap();
    assert!(started.elapsed() >= Duration::from_millis(180));
}

#[tokio::test]
async fn total_deadline_includes_retry_after() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/getDataCode"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "60"))
        .expect(1)
        .mount(&server)
        .await;
    let mut settings = config();
    settings.total_timeout = Duration::from_millis(30);
    let error = client(&server, settings)
        .get_data_code(&query())
        .await
        .unwrap_err();
    assert_eq!(error, BojError::DeadlineExceeded);
}

#[tokio::test]
async fn per_attempt_timeouts_are_retried_and_preserve_their_kind() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/getDataCode"))
        .respond_with(success().set_delay(Duration::from_millis(200)))
        .expect(2)
        .mount(&server)
        .await;
    let mut settings = config();
    settings.request_timeout = Duration::from_millis(30);
    settings.max_attempts = 2;
    let error = client(&server, settings)
        .get_data_code(&query())
        .await
        .unwrap_err();
    assert!(
        matches!(error, BojError::RetryExhausted { attempts: 2, last }
        if matches!(*last, BojError::TransportError {
            kind: boj_client::error::TransportErrorKind::Timeout, ..
        }))
    );
}

#[tokio::test]
async fn invalid_request_is_rejected_without_retrying() {
    let client = BojClient::with_config(config())
        .unwrap()
        .with_base_url("not a URL");
    let error = client.get_data_code(&query()).await.unwrap_err();
    assert!(matches!(
        error,
        BojError::TransportError {
            kind: boj_client::error::TransportErrorKind::Request,
            ..
        }
    ));
}

#[tokio::test]
async fn dropping_an_in_flight_future_releases_its_slot() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/getDataCode"))
        .respond_with(success().set_delay(Duration::from_secs(10)))
        .mount(&server)
        .await;
    let mut settings = config();
    settings.max_concurrent_requests = 1;
    let client = client(&server, settings);
    let clone = client.clone();
    let task = tokio::spawn(async move { clone.get_data_code(&query()).await });
    tokio::time::timeout(Duration::from_secs(2), async {
        while server.received_requests().await.unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    server.reset().await;
    Mock::given(path("/api/v1/getDataCode"))
        .respond_with(success())
        .mount(&server)
        .await;
    tokio::time::timeout(Duration::from_secs(2), client.get_data_code(&query()))
        .await
        .unwrap()
        .unwrap();
}

#[test]
fn invalid_configuration_is_rejected() {
    for settings in [
        BojResilienceConfig {
            burst_size: 0,
            ..config()
        },
        BojResilienceConfig {
            max_attempts: 0,
            ..config()
        },
        BojResilienceConfig {
            max_concurrent_requests: 0,
            ..config()
        },
        BojResilienceConfig {
            max_pages: 0,
            ..config()
        },
        BojResilienceConfig {
            request_timeout: Duration::ZERO,
            ..config()
        },
        BojResilienceConfig {
            total_timeout: Duration::ZERO,
            ..config()
        },
    ] {
        assert!(matches!(
            BojClient::with_config(settings),
            Err(BojError::ValidationError(_))
        ));
    }
}
