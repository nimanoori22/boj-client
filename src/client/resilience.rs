use std::time::Duration;

use crate::error::BojError;

/// Retry, rate, concurrency, and timeout settings shared across client clones.
///
/// The default rate is a conservative client choice, not a BOJ-published quota.
#[derive(Debug, Clone)]
pub struct BojResilienceConfig {
    /// Requests per second. Zero disables rate limiting.
    pub requests_per_second: u32,
    /// Initial rate-limit burst size (must be positive).
    pub burst_size: u32,
    /// Maximum simultaneous HTTP attempts (must be positive).
    pub max_concurrent_requests: usize,
    /// Maximum attempts per page, including the initial attempt.
    pub max_attempts: u32,
    /// Initial exponential backoff delay. Rustopus adds jitter.
    pub initial_backoff: Duration,
    /// Upper bound on exponential backoff.
    pub max_backoff: Duration,
    /// Timeout for each HTTP attempt, including reading the response body.
    pub request_timeout: Duration,
    /// Deadline per page, covering queueing, all attempts, and backoff.
    pub total_timeout: Duration,
    /// Maximum server-directed pause from `Retry-After`.
    pub max_retry_after: Duration,
    /// Maximum pages for a complete-history call. Reaching it with more data fails.
    pub max_pages: usize,
}

impl Default for BojResilienceConfig {
    fn default() -> Self {
        Self {
            requests_per_second: 1,
            burst_size: 1,
            max_concurrent_requests: 2,
            max_attempts: 4,
            initial_backoff: Duration::from_millis(500),
            max_backoff: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            total_timeout: Duration::from_secs(180),
            max_retry_after: Duration::from_secs(60),
            max_pages: 1000,
        }
    }
}

impl BojResilienceConfig {
    pub(super) fn validate(&self) -> Result<(), BojError> {
        if self.burst_size == 0
            || self.max_concurrent_requests == 0
            || self.max_attempts == 0
            || self.max_pages == 0
        {
            return Err(BojError::validation(
                "burst size, concurrency, attempts, and page limit must be positive",
            ));
        }
        if self.request_timeout.is_zero() || self.total_timeout.is_zero() {
            return Err(BojError::validation(
                "request and total timeouts must be positive",
            ));
        }
        if self.max_backoff < self.initial_backoff {
            return Err(BojError::validation(
                "maximum backoff must be at least the initial backoff",
            ));
        }
        Ok(())
    }
}
