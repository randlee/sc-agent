//! Checked per-call request timeout shared by the decidr and Python send APIs.

use std::time::Duration;

use crate::DecidrError;
use sc_agent_config::{MAX_WORKERS, RUNNER_REQUEST_TIMEOUT_SECS};

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
            return Err(DecidrError::InvalidRequestTimeout { value: seconds });
        }
        Ok(Self(Duration::from_secs_f64(seconds)))
    }

    pub(crate) fn duration(self) -> Duration {
        self.0
    }
}

/// Validated per-call batch worker count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Workers(usize);

impl Workers {
    /// Validate a worker count within 1..=`MAX_WORKERS`.
    ///
    /// # Errors
    /// Returns `decidr.invalid_workers` if outside the supported range.
    pub fn new(count: i64) -> Result<Self, DecidrError> {
        let Ok(workers) = usize::try_from(count) else {
            return Err(DecidrError::InvalidWorkers { value: count });
        };
        if !(1..=MAX_WORKERS).contains(&workers) {
            return Err(DecidrError::InvalidWorkers { value: count });
        }
        Ok(Self(workers))
    }

    pub(crate) fn get(self) -> usize {
        self.0
    }
}

/// Optional per-call request and batch concurrency limits.
#[derive(Debug, Clone, Copy, Default)]
pub struct CallLimits {
    /// Per-request timeout.
    pub timeout: Option<RequestTimeout>,
    /// Batch worker count.
    pub workers: Option<Workers>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_agent_types::ErrorId;

    #[test]
    fn workers_are_validated() {
        let max_workers = i64::try_from(MAX_WORKERS).expect("MAX_WORKERS fits in i64");
        for count in [1, 8, max_workers] {
            assert_eq!(
                Workers::new(count).unwrap().get(),
                usize::try_from(count).unwrap()
            );
        }
        for count in [i64::MIN, 0, max_workers + 1, i64::MAX] {
            let error = Workers::new(count).unwrap_err();
            assert_eq!(error.id(), "decidr.invalid_workers");
            assert_eq!(
                error.message(),
                format!(
                    "workers must be within 1..={MAX_WORKERS}; CLI --workers requires --batch, got {count}"
                )
            );
        }
    }

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
            assert!(error.message().contains(&seconds.to_string()));
            assert!(matches!(error, DecidrError::InvalidRequestTimeout { .. }));
            assert!(error.cause_detail().is_none());
        }
    }
}
