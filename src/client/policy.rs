use std::time::Duration;

use rustopus::{Decision, Outcome, Policy, TransportError, TransportErrorKind};

use crate::{decode::api_error, error::BojError};

use super::response::normalize_response_body;

pub(super) struct BojPolicy {
    pub max_retry_after: Duration,
}

impl BojPolicy {
    fn retry_after(&self, outcome: &Outcome) -> Option<Duration> {
        // Rustopus currently supports delta seconds, not HTTP-date values.
        let seconds: u64 = outcome
            .headers
            .get("retry-after")?
            .to_str()
            .ok()?
            .trim()
            .parse()
            .ok()?;
        Some(Duration::from_secs(seconds).min(self.max_retry_after))
    }
}

impl Policy for BojPolicy {
    type Error = BojError;

    fn decide(
        &self,
        _attempt: u32,
        result: &Result<Outcome, TransportError>,
    ) -> Decision<BojError> {
        let outcome = match result {
            Err(error) => {
                return match error.kind() {
                    TransportErrorKind::Timeout
                    | TransportErrorKind::Connect
                    | TransportErrorKind::Body => Decision::Retry { after: None },
                    _ => Decision::Accept,
                };
            }
            Ok(outcome) => outcome,
        };

        if matches!(outcome.status.as_u16(), 408 | 429 | 500 | 502 | 503 | 504) {
            return Decision::Retry {
                after: self.retry_after(outcome),
            };
        }
        if !outcome.status.is_success() {
            return Decision::Fail(response_error(outcome));
        }

        let body = match normalize_response_body(outcome) {
            Ok(body) => body,
            Err(error) => return Decision::Fail(error),
        };
        match api_error(&body) {
            Ok(Some(BojError::ApiError {
                status: 500 | 503, ..
            })) => Decision::Retry {
                after: self.retry_after(outcome),
            },
            Ok(Some(error)) | Err(error) => Decision::Fail(error),
            Ok(None) => Decision::Accept,
        }
    }
}

pub(super) fn response_error(outcome: &Outcome) -> BojError {
    if let Ok(body) = normalize_response_body(outcome)
        && let Ok(Some(error)) = api_error(&body)
    {
        return error;
    }
    BojError::HttpError {
        status: outcome.status.as_u16(),
        headers: outcome
            .headers
            .iter()
            .map(|(name, value)| {
                (
                    name.to_string(),
                    String::from_utf8_lossy(value.as_bytes()).into_owned(),
                )
            })
            .collect(),
        body: outcome.body.to_vec(),
    }
}

fn transport_error(error: TransportError) -> BojError {
    BojError::TransportError {
        kind: error.kind(),
        message: error.to_string(),
    }
}

pub(super) fn map_arm_error(error: rustopus::Error<BojError>) -> BojError {
    match error {
        rustopus::Error::Transport(error) => transport_error(error),
        rustopus::Error::Policy(error) => error,
        rustopus::Error::Deadline => BojError::DeadlineExceeded,
        rustopus::Error::Exhausted { attempts, last } => BojError::RetryExhausted {
            attempts,
            last: Box::new(match *last {
                Ok(outcome) => response_error(&outcome),
                Err(error) => transport_error(error),
            }),
        },
        other => BojError::transport(other.to_string()),
    }
}
