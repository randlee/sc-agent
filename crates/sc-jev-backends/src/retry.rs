//! Busy-retry wait and the HTTP failure text for provider calls.
//!
//! The policy itself (busy statuses, `Retry-After` parsing, jitter, the
//! deadline, and the exhausted wording) is [`crate::retry_policy`], shared
//! verbatim with `sc-jev`. Four sends total, then
//! [`JevError::NetworkError`](sc_agent_types::JevError::NetworkError).

use std::time::Duration;

use sc_agent_types::JevError;

use crate::transport::HttpResponse;

/// How many times one provider call is sent, including the first attempt.
///
/// Matches the packaged Jev `attempts` value so Haiku and Luna share that
/// busy policy (REQ-SAG-0505).
pub(crate) const BUSY_ATTEMPTS: u32 = 4;

/// Sleeps between busy retries.
///
/// This trait is an open extension point (ADR-SAG-0006) so tests can record
/// waits without sleeping the calling thread. It is intentionally not sealed.
/// The production sleeper blocks the calling thread. `delay` already
/// includes the clamp and jitter from the busy-retry policy.
pub trait RetrySleeper: Send + Sync {
    /// Waits `delay` before the next send of the same body.
    fn wait(&self, delay: Duration);
}

pub(crate) struct ThreadSleeper;

impl RetrySleeper for ThreadSleeper {
    fn wait(&self, delay: Duration) {
        if !delay.is_zero() {
            std::thread::sleep(delay);
        }
    }
}

/// `Retry-After` header value, if the provider sent one.
pub(crate) fn retry_after_header(response: &HttpResponse) -> Option<&str> {
    response
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("retry-after"))
        .map(|(_, value)| value.as_str())
}

pub(crate) fn http_failure(provider: &str, response: &HttpResponse) -> JevError {
    JevError::NetworkError {
        detail: format!("{provider} HTTP {}: {}", response.status, snippet(response)),
    }
}

fn snippet(response: &HttpResponse) -> String {
    let end = response.body.len().min(300);
    String::from_utf8_lossy(&response.body[..end]).replace(['\n', '\r'], " ")
}
