//! The one `Retry-After` parser, with two views of its result.
//!
//! Valid is 1 to 32 bytes of ASCII digits with an optional `.` and ASCII-digit
//! fraction. Anything else (an HTTP-date, a sign, an exponent, `inf`, `NaN`,
//! empty, longer than 32 bytes) is malformed and gives `None`. The wait uses
//! no floating point: the integer part saturates and only the first nine
//! fraction digits count toward it.
//!
//! - [`RetryAfter::wait`] is the provider's wait, clamped to 0-30 s (the
//!   resend schedule adds its floor and jitter, REQ-SAG-0012).
//! - [`RetryAfter::seconds`] is what an error reports: the value as sent,
//!   unrounded (REQ-SAG-0007).

use std::time::Duration;

/// Upper clamp of an honoured `Retry-After`.
pub(crate) const WAIT_MAX: Duration = Duration::from_secs(30);

/// Longest valid `Retry-After` value, in bytes.
const MAX_LEN: usize = 32;

/// A parsed, valid `Retry-After` value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RetryAfter {
    seconds: u64,
    nanos: u32,
    exact: f64,
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
        // Digits with an optional digit fraction always parse as an f64.
        let exact = text.parse::<f64>().ok()?;
        Some(Self {
            seconds,
            nanos,
            exact,
        })
    }

    /// The wait to honour, clamped to 0-30 s.
    pub(crate) fn wait(self) -> Duration {
        if self.seconds >= WAIT_MAX.as_secs() {
            return WAIT_MAX;
        }
        Duration::new(self.seconds, self.nanos).min(WAIT_MAX)
    }

    /// The value as sent, in seconds, unrounded and unclamped (REQ-SAG-0007).
    pub(crate) fn seconds(self) -> f64 {
        self.exact
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_are_unrounded_and_unclamped() {
        let seconds = |text: &str| RetryAfter::parse(Some(text)).map(RetryAfter::seconds);
        assert_eq!(seconds("2.5"), Some(2.5));
        assert_eq!(seconds("0.001"), Some(0.001));
        assert_eq!(seconds("120"), Some(120.0));
        assert_eq!(seconds("abc"), None);
    }
}
