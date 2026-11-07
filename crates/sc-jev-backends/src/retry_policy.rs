//! Busy-retry policy shared by the Jev and LLM clients.
//!
//! This file is copied verbatim into `sc-jev` and `sc-jev-backends` (interim
//! until the crate-layout decision D5). Each copy runs the same table test, so
//! the two cannot drift silently. Edit both copies together.
//!
//! - Busy statuses are [`BUSY_STATUSES`]: 429, 502, and 529. The body is
//!   resent unchanged.
//! - `Retry-After` is integer or decimal delta-seconds, clamped to 0..=30.
//!   A missing header waits 1 s. A present header that is not a finite
//!   number (empty, an HTTP date, `NaN`) waits 30 s.
//! - Up to [`JITTER_MAX`] of jitter is added after the clamp, so clients that
//!   were throttled together do not retry in lock step.
//! - Every retry loop stops at [`RETRY_DEADLINE`] measured from the first
//!   send: a wait is shortened to the time left, and no wait starts once it
//!   is spent.
//! - Running out of attempts or deadline is `NETWORK_ERROR` with the
//!   [`exhausted_detail`] wording.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::{Duration, Instant};

/// HTTP statuses that mean "busy, retry the same body".
pub(crate) const BUSY_STATUSES: [u16; 3] = [429, 502, 529];

/// Upper bound for one `Retry-After` wait, before jitter.
pub(crate) const RETRY_AFTER_MAX: Duration = Duration::from_secs(30);

/// Wait when the busy reply has no `Retry-After` header.
pub(crate) const RETRY_AFTER_MISSING: Duration = Duration::from_secs(1);

/// Wait when `Retry-After` is present but is not a finite number of seconds.
pub(crate) const RETRY_AFTER_MALFORMED: Duration = Duration::from_secs(30);

/// Largest jitter added to a wait.
pub(crate) const JITTER_MAX: Duration = Duration::from_millis(250);

/// Total time budget for one retry loop, measured from the first send.
///
/// Three maximal `Retry-After` waits fit, so the deadline only bites when the
/// sends themselves are slow.
pub(crate) const RETRY_DEADLINE: Duration = Duration::from_secs(90);

/// Whether `status` is a busy reply that is retried.
pub(crate) fn is_busy(status: u16) -> bool {
    BUSY_STATUSES.contains(&status)
}

/// Parses a raw `Retry-After` header into the wait before jitter.
pub(crate) fn retry_after(header: Option<&str>) -> Duration {
    let Some(text) = header else {
        return RETRY_AFTER_MISSING;
    };
    match text.trim().parse::<f64>() {
        Ok(seconds) if seconds.is_finite() => {
            Duration::from_secs_f64(seconds.clamp(0.0, RETRY_AFTER_MAX.as_secs_f64()))
        }
        _ => RETRY_AFTER_MALFORMED,
    }
}

/// `NETWORK_ERROR` detail when busy retries stop.
pub(crate) fn exhausted_detail(provider: &str, status: u16, sends: u32) -> String {
    format!("{provider} busy: HTTP {status} after {sends} sends; retry attempts exhausted")
}

/// Deadline tracker for one retry loop.
pub(crate) struct RetryBudget {
    started: Instant,
}

impl RetryBudget {
    /// Starts the budget. Call this immediately before the first send.
    pub(crate) fn start() -> Self {
        Self {
            started: Instant::now(),
        }
    }

    /// Returns the next wait, or `None` when the deadline is spent.
    pub(crate) fn next_wait(&self, requested: Duration) -> Option<Duration> {
        bounded_wait(requested, self.started.elapsed(), jitter())
    }
}

/// Adds `jitter` to `requested` and caps it at the time left before
/// [`RETRY_DEADLINE`]. `None` means no time is left.
pub(crate) fn bounded_wait(
    requested: Duration,
    elapsed: Duration,
    jitter: Duration,
) -> Option<Duration> {
    let remaining = RETRY_DEADLINE.checked_sub(elapsed)?;
    if remaining.is_zero() {
        return None;
    }
    Some(requested.saturating_add(jitter).min(remaining))
}

/// Pseudo-random jitter in `0..=JITTER_MAX`.
///
/// `RandomState` is seeded per process and per call, which is enough to
/// de-synchronize clients; this is not a cryptographic source.
fn jitter() -> Duration {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(0);
    let max_millis = u64::try_from(JITTER_MAX.as_millis()).unwrap_or(u64::MAX);
    Duration::from_millis(hasher.finish() % (max_millis + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Same table in both crates: `(header, wait before jitter)`.
    const RETRY_AFTER_TABLE: &[(Option<&str>, Duration)] = &[
        (None, Duration::from_secs(1)),
        (Some("0"), Duration::ZERO),
        (Some("2"), Duration::from_secs(2)),
        (Some(" 7 "), Duration::from_secs(7)),
        (Some("2.5"), Duration::from_millis(2_500)),
        (Some("30"), Duration::from_secs(30)),
        (Some("45"), Duration::from_secs(30)),
        (Some("1e9"), Duration::from_secs(30)),
        (Some("-4"), Duration::ZERO),
        (Some(""), Duration::from_secs(30)),
        (Some("   "), Duration::from_secs(30)),
        (Some("soon"), Duration::from_secs(30)),
        (Some("NaN"), Duration::from_secs(30)),
        (Some("inf"), Duration::from_secs(30)),
        (
            Some("Wed, 21 Oct 2026 07:28:00 GMT"),
            Duration::from_secs(30),
        ),
    ];

    #[test]
    fn retry_after_table() {
        for (header, expected) in RETRY_AFTER_TABLE {
            assert_eq!(retry_after(*header), *expected, "{header:?}");
        }
    }

    #[test]
    fn busy_status_set() {
        let busy: Vec<u16> = (100..600).filter(|status| is_busy(*status)).collect();
        assert_eq!(busy, BUSY_STATUSES);
    }

    #[test]
    fn exhausted_wording() {
        assert_eq!(
            exhausted_detail("haiku", 429, 4),
            "haiku busy: HTTP 429 after 4 sends; retry attempts exhausted"
        );
    }

    #[test]
    fn deadline_caps_and_stops_waits() {
        let second = Duration::from_secs(1);
        let jitter = Duration::from_millis(100);
        assert_eq!(
            bounded_wait(second, Duration::ZERO, jitter),
            Some(second + jitter)
        );
        assert_eq!(
            bounded_wait(RETRY_AFTER_MAX, RETRY_DEADLINE - second, jitter),
            Some(second)
        );
        assert_eq!(bounded_wait(second, RETRY_DEADLINE, jitter), None);
        assert_eq!(bounded_wait(second, RETRY_DEADLINE + second, jitter), None);
    }

    #[test]
    fn jitter_stays_in_range() {
        for _ in 0..64 {
            assert!(jitter() <= JITTER_MAX);
        }
        let budget = RetryBudget::start();
        let wait = budget.next_wait(second()).expect("budget left");
        assert!(wait >= second() && wait <= second() + JITTER_MAX);
    }

    fn second() -> Duration {
        Duration::from_secs(1)
    }
}
