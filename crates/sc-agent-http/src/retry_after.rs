//! The one `Retry-After` parser, with two views of its result.
//!
//! Valid is 1 to 32 bytes of ASCII digits with an optional `.` and ASCII-digit
//! fraction. Anything else (an HTTP-date, a sign, an exponent, `inf`, `NaN`,
//! empty, longer than 32 bytes) is malformed and gives `None`. No floating
//! point is involved: the integer part saturates and only the first nine
//! fraction digits count toward the wait.
//!
//! - [`RetryAfter::wait`] is what the retry loop honours: clamped to 0-30 s.
//! - [`RetryAfter::report_seconds`] is what an error reports: whole seconds,
//!   a fraction rounded up, capped at one day, so a long provider wait is not
//!   shown as the 30 s clamp.

use std::time::Duration;

/// Upper clamp of an honoured `Retry-After`.
pub(crate) const WAIT_MAX: Duration = Duration::from_secs(30);

/// Largest value an error reports, in seconds (one day).
pub(crate) const REPORT_MAX_SECONDS: u64 = 86_400;

/// Longest valid `Retry-After` value, in bytes.
const MAX_LEN: usize = 32;

/// A parsed, valid `Retry-After` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RetryAfter {
    seconds: u64,
    nanos: u32,
    fraction_is_nonzero: bool,
}

impl RetryAfter {
    /// `None` when `header` is absent or malformed.
    pub(crate) fn parse(header: Option<&str>) -> Option<Self> {
        let text = header?;
        if text.is_empty() || text.len() > MAX_LEN {
            return None;
        }
        let (whole, fraction) = match text.split_once('.') {
            Some((whole, fraction)) => (whole, Some(fraction)),
            None => (text, None),
        };
        let all_digits =
            |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
        if !all_digits(whole) || fraction.is_some_and(|part| !all_digits(part)) {
            return None;
        }
        let seconds = whole.bytes().fold(0_u64, |acc, byte| {
            acc.saturating_mul(10)
                .saturating_add(u64::from(byte - b'0'))
        });
        let nanos = fraction.map_or(0, |part| {
            part.bytes()
                .chain(std::iter::repeat(b'0'))
                .take(9)
                .fold(0_u32, |acc, byte| acc * 10 + u32::from(byte - b'0'))
        });
        let fraction_is_nonzero = fraction.is_some_and(|part| part.bytes().any(|b| b != b'0'));
        Some(Self {
            seconds,
            nanos,
            fraction_is_nonzero,
        })
    }

    /// The wait to honour, clamped to 0-30 s.
    pub(crate) fn wait(self) -> Duration {
        if self.seconds >= WAIT_MAX.as_secs() {
            return WAIT_MAX;
        }
        Duration::new(self.seconds, self.nanos).min(WAIT_MAX)
    }

    /// Whole seconds to report: a fraction rounds up, capped at one day.
    pub(crate) fn report_seconds(self) -> u64 {
        self.seconds
            .saturating_add(u64::from(self.fraction_is_nonzero))
            .min(REPORT_MAX_SECONDS)
    }
}
