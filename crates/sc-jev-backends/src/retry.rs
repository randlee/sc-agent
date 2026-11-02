//! Busy retry shared by token-count and judge calls.
//!
//! Statuses 429, 502, and 529 are retried. The body is resent unchanged.
//! `Retry-After` is a delta-seconds integer clamped to 0..=30. A missing
//! header waits 1s. An unparsable header waits 30s. Four sends total, then
//! [`JevError::NetworkError`](sc_agent_types::JevError::NetworkError).

use std::time::Duration;

use sc_agent_types::JevError;

use crate::transport::HttpResponse;

/// How many times one provider call is sent, including the first attempt.
///
/// Matches the packaged Jev `attempts` value so Haiku and Luna share that
/// busy policy (REQ-SAG-0505).
pub(crate) const BUSY_ATTEMPTS: u32 = 4;

/// Upper bound for a single `Retry-After` wait.
///
/// Clamped so a provider cannot stall the process for longer than half a minute.
const RETRY_AFTER_MAX_SECS: i64 = 30;

/// Wait used when `Retry-After` is absent.
const RETRY_AFTER_MISSING: Duration = Duration::from_secs(1);

/// Wait used when `Retry-After` is not an integer number of seconds.
const RETRY_AFTER_UNPARSABLE: Duration = Duration::from_secs(30);

/// Sleeps between busy retries.
///
/// The production sleeper blocks the calling thread. Tests supply a sleeper
/// that records the delay and returns.
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

pub(crate) fn is_busy(status: u16) -> bool {
    matches!(status, 429 | 502 | 529)
}

pub(crate) fn retry_delay(response: &HttpResponse) -> Duration {
    let Some((_, value)) = response
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("retry-after"))
    else {
        return RETRY_AFTER_MISSING;
    };
    match value.trim().parse::<i64>() {
        Ok(seconds) => {
            Duration::from_secs(u64::try_from(seconds.clamp(0, RETRY_AFTER_MAX_SECS)).unwrap_or(0))
        }
        Err(_) => RETRY_AFTER_UNPARSABLE,
    }
}

pub(crate) fn exhausted(provider: &str, response: &HttpResponse) -> JevError {
    JevError::NetworkError {
        detail: format!(
            "{provider} busy attempts exhausted after {BUSY_ATTEMPTS} sends: HTTP {}: {}",
            response.status,
            snippet(response)
        ),
    }
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
