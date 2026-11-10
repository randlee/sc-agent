//! Busy resend schedule: busy statuses, backoff and jitter (REQ-SAG-0601,
//! REQ-SAG-0012).

use std::hash::{BuildHasher, RandomState};
use std::time::{Duration, Instant};

/// HTTP statuses that mean "busy, resend the same body".
const BUSY_STATUSES: [u16; 3] = [429, 502, 529];

/// First backoff wait when `Retry-After` is absent or malformed.
const BACKOFF_FIRST: Duration = Duration::from_secs(1);

/// Cap of the exponential backoff.
const BACKOFF_MAX: Duration = Duration::from_secs(30);

/// Largest jitter added to a holdoff.
const JITTER_MAX: Duration = Duration::from_millis(250);

/// Whether `status` is a busy status (429, 502, 529).
pub(crate) fn is_busy_status(status: u16) -> bool {
    BUSY_STATUSES.contains(&status)
}

/// Jitter in `0..=JITTER_MAX` (0-250 ms).
///
/// Hashes the current time with a fresh std `RandomState`: enough to
/// de-synchronise clients, not a cryptographic source.
fn jitter() -> Duration {
    let max = u64::try_from(JITTER_MAX.as_millis()).unwrap_or(u64::MAX);
    let hash = RandomState::new().hash_one(Instant::now());
    Duration::from_millis(hash % (max + 1))
}

/// Backoff before the `nth` resend (1-based): 1, 2, 4, 8 … s, capped at 30 s.
fn backoff(nth: u32) -> Duration {
    let factor = 1_u32.checked_shl(nth.saturating_sub(1)).unwrap_or(u32::MAX);
    BACKOFF_FIRST.saturating_mul(factor).min(BACKOFF_MAX)
}
