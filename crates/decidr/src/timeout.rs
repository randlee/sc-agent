//! Checked per-call request timeout shared by the decidr and Python send APIs.

use std::time::Duration;

use sc_agent_config::RUNNER_REQUEST_TIMEOUT_SECS;
use sc_agent_types::ErrorDetail;

use crate::DecidrError;

/// A finite per-call request timeout within the configured runner request bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestTimeout(Duration);

impl RequestTimeout {
    /// Checks seconds against the existing 10..=3600 request-timeout bounds.
    /// Fractional seconds are retained for callers with floating-point CLI options.
    ///
    /// # Errors
    /// Returns [`DecidrError::InvalidRequestTimeout`] for non-finite or out-of-range values.
    pub fn from_secs(seconds: f64) -> Result<Self, DecidrError> {
        let min = f64::from(*RUNNER_REQUEST_TIMEOUT_SECS.start());
        let max = f64::from(*RUNNER_REQUEST_TIMEOUT_SECS.end());
        if !seconds.is_finite() || !(min..=max).contains(&seconds) {
            return Err(DecidrError::InvalidRequestTimeout { source: None });
        }
        Duration::try_from_secs_f64(seconds)
            .map(Self)
            .map_err(|source| DecidrError::InvalidRequestTimeout {
                source: Some(Box::new(ErrorDetail::foreign(
                    "std.time.float_duration",
                    &source,
                ))),
            })
    }

    pub(crate) fn duration(self) -> Duration {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_agent_types::ErrorId;

    #[test]
    fn finite_integer_and_fractional_seconds_are_checked_without_panics() {
        for seconds in [10.0, 30.0, 30.5, 3600.0] {
            assert_eq!(
                RequestTimeout::from_secs(seconds)
                    .unwrap()
                    .duration()
                    .as_secs_f64(),
                seconds
            );
        }
        for seconds in [
            0.0,
            -1.0,
            9.99,
            3600.01,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            let error = RequestTimeout::from_secs(seconds).unwrap_err();
            assert_eq!(error.id(), "decidr.invalid_request_timeout");
            assert!(error.message().contains("timeout_secs"));
            assert!(error.message().contains("10..=3600"));
        }
    }
}
