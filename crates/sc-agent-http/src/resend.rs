/// HTTP statuses that mean "busy, resend the same body".
const BUSY_STATUSES: [u16; 3] = [429, 502, 529];

/// Sends per call under [`BusyPolicy::default`], including the first.
const DEFAULT_MAX_SENDS: u32 = 5;

/// Most sends per call under any policy (RSH-002). [`BusyPolicy::new`] and
/// [`BusyPolicy::with_max_sends`] clamp to `1..=MAX_SENDS`.
const MAX_SENDS: u32 = 5;

/// First backoff wait when `Retry-After` is absent or malformed.
const BACKOFF_FIRST: Duration = Duration::from_secs(1);

/// Cap of the exponential backoff.
const BACKOFF_MAX: Duration = Duration::from_secs(30);

/// Largest jitter added to a wait.
const JITTER_MAX: Duration = Duration::from_millis(250);

/// Budget for starting attempts, measured from the first send.
const BUDGET: Duration = Duration::from_secs(90);
    /// Jitter in `0..=JITTER_MAX`.
    ///
    /// Production hashes the current time with a fresh std `RandomState`:
    /// enough to de-synchronise clients, not a cryptographic source.
    fn jitter(&self) -> Duration {
        match self {
            Self::System => {
                let max = u64::try_from(JITTER_MAX.as_millis()).unwrap_or(u64::MAX);
                let hash = RandomState::new().hash_one(Instant::now());
                Duration::from_millis(hash % (max + 1))
            }
            #[cfg(feature = "test-util")]
            Self::Test { jitter, .. } => *jitter,
        }
    }
/// Whether `response` is a busy reply that is worth resending.
///
/// The classification hook: a busy status ([`BUSY_STATUSES`]) whose body
/// says the account is out of quota is not busy, because waiting cannot
/// help. Retry timing is unaffected.
fn is_busy(response: &HttpResponse) -> bool {
    BUSY_STATUSES.contains(&response.status())
        && !is_quota_reply(response.status(), response.body())
}

fn plural(sends: u32) -> &'static str {
    if sends == 1 { "send" } else { "sends" }
}

fn cancelled(when: &str) -> JevError {
    JevError::network(
        NetworkErrorKind::Cancelled,
        format!("request cancelled {when}"),
    )
}

/// Backoff before the `nth` wait (1-based): 1, 2, 4, 8 … s, capped at 30 s.
fn backoff(nth: u32) -> Duration {
    let factor = 1_u32.checked_shl(nth.saturating_sub(1)).unwrap_or(u32::MAX);
    BACKOFF_FIRST.saturating_mul(factor).min(BACKOFF_MAX)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::retry_after::WAIT_MAX as RETRY_AFTER_MAX;

    #[test]
    fn backoff_doubles_to_the_cap() {
        let seconds: Vec<u64> = (1..=8).map(|nth| backoff(nth).as_secs()).collect();
        assert_eq!(seconds, [1, 2, 4, 8, 16, 30, 30, 30]);
        assert_eq!(backoff(u32::MAX), BACKOFF_MAX);
        assert_eq!(backoff(0), BACKOFF_FIRST);
    }

    #[test]
    fn retry_after_parser() {
        let cases: &[(Option<&str>, Option<Duration>)] = &[
            (None, None),
            (Some("3"), Some(Duration::from_secs(3))),
            (Some("2.5"), Some(Duration::from_millis(2_500))),
            (Some("0"), Some(Duration::ZERO)),
            (Some("0.000000001"), Some(Duration::from_nanos(1))),
            (Some("1.1234567899"), Some(Duration::new(1, 123_456_789))),
            (Some("29.999"), Some(Duration::from_millis(29_999))),
            (Some("30"), Some(RETRY_AFTER_MAX)),
            (Some("30.5"), Some(RETRY_AFTER_MAX)),
            (Some(&"9".repeat(32)), Some(RETRY_AFTER_MAX)),
            (Some(&"9".repeat(33)), None),
            (Some(""), None),
            (Some(" 7"), None),
            (Some("-1"), None),
            (Some("+1"), None),
            (Some("1e9"), None),
            (Some("inf"), None),
            (Some("NaN"), None),
            (Some("1."), None),
            (Some(".5"), None),
            (Some("1.2.3"), None),
            (Some("Wed, 21 Oct 2026 07:28:00 GMT"), None),
        ];
        for (header, expected) in cases {
            assert_eq!(parse_retry_after(*header), *expected, "{header:?}");
        }
    }

    #[test]
    fn system_jitter_stays_in_range() {
        for _ in 0..64 {
            assert!(Hooks::System.jitter() <= JITTER_MAX);
        }
    }

    #[test]
    fn default_delays_are_one_two_four_eight() {
        let delays: Vec<u64> = (1..DEFAULT_MAX_SENDS)
            .map(|nth| backoff(nth).as_secs())
            .collect();
        assert_eq!(delays, [1, 2, 4, 8]);
        assert_eq!(delays.iter().sum::<u64>(), 15);
    }

    #[test]
    fn busy_set_is_exactly_three_statuses() {
        let busy: Vec<u16> = (100..600)
            .filter(|status| BUSY_STATUSES.contains(status))
            .collect();
        assert_eq!(busy, [429, 502, 529]);
    }
}
