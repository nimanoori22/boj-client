use thiserror::Error;

/// Classification of an HTTP transport failure, shared with Rustopus.
pub use rustopus::TransportErrorKind;

/// Error type returned by the BOJ client.
///
/// # Examples
///
/// ```
/// use boj_client::error::BojError;
///
/// let error = BojError::validation("invalid parameter");
/// assert!(matches!(error, BojError::ValidationError(_)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BojError {
    /// Input validation failed before sending a request.
    #[error("validation error: {0}")]
    ValidationError(String),

    /// Response decoding failed for JSON/CSV payloads.
    #[error("decode error: {0}")]
    DecodeError(String),

    /// Transport-level failure (network, HTTP client, invalid request shape).
    #[error("transport error ({kind:?}): {message}")]
    TransportError {
        /// Whether the failure was a timeout, connection, body, or request failure.
        kind: TransportErrorKind,
        /// Diagnostic message from the underlying transport.
        message: String,
    },

    /// HTTP failure without a BOJ API error payload.
    #[error("HTTP {status}")]
    HttpError {
        /// HTTP status code.
        status: u16,
        /// Response headers preserved for diagnostics.
        headers: std::collections::BTreeMap<String, String>,
        /// Raw response bytes preserved for diagnostics.
        body: Vec<u8>,
    },

    /// All configured attempts failed.
    #[error("gave up after {attempts} attempts: {last}")]
    RetryExhausted {
        /// Attempts made, including the initial request.
        attempts: u32,
        /// Final HTTP, BOJ API, or transport failure.
        #[source]
        last: Box<BojError>,
    },

    /// Total request deadline elapsed, including queueing and retry delays.
    #[error("total request deadline exceeded")]
    DeadlineExceeded,

    /// Pagination could not finish safely; no partial result is returned.
    #[error("incomplete pagination: {0}")]
    PaginationError(String),

    /// API-level error represented by BOJ status and message fields.
    #[error("api error: status={status}, message_id={message_id}, message={message}")]
    ApiError {
        /// BOJ `STATUS` value.
        status: u16,
        /// BOJ `MESSAGE-ID` value.
        message_id: String,
        /// BOJ `MESSAGE` value.
        message: String,
    },
}

impl BojError {
    /// Creates [`BojError::ValidationError`].
    ///
    /// # Examples
    ///
    /// ```
    /// use boj_client::error::BojError;
    ///
    /// let error = BojError::validation("invalid db");
    /// assert!(matches!(error, BojError::ValidationError(_)));
    /// ```
    pub fn validation(message: impl Into<String>) -> Self {
        Self::ValidationError(message.into())
    }

    /// Creates [`BojError::DecodeError`].
    ///
    /// # Examples
    ///
    /// ```
    /// use boj_client::error::BojError;
    ///
    /// let error = BojError::decode("invalid JSON");
    /// assert!(matches!(error, BojError::DecodeError(_)));
    /// ```
    pub fn decode(message: impl Into<String>) -> Self {
        Self::DecodeError(message.into())
    }

    /// Creates [`BojError::TransportError`].
    ///
    /// # Examples
    ///
    /// ```
    /// use boj_client::error::BojError;
    ///
    /// let error = BojError::transport("timeout");
    /// assert!(matches!(error, BojError::TransportError { .. }));
    /// ```
    pub fn transport(message: impl Into<String>) -> Self {
        Self::TransportError {
            kind: TransportErrorKind::Other,
            message: message.into(),
        }
    }

    /// Creates [`BojError::ApiError`].
    ///
    /// # Examples
    ///
    /// ```
    /// use boj_client::error::BojError;
    ///
    /// let error = BojError::api(500, "M181090S", "internal");
    /// assert!(matches!(error, BojError::ApiError { status: 500, .. }));
    /// ```
    pub fn api(status: u16, message_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ApiError {
            status,
            message_id: message_id.into(),
            message: message.into(),
        }
    }
}
