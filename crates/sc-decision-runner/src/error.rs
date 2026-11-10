//! [`RunnerError`]: the error of the runner's own fallible method.

use std::error::Error;
use std::fmt;

use sc_agent_log::LogError;
use sc_agent_types::{ErrorDetail, ErrorId};

/// Why [`DecisionRunner::flush_session_log`](crate::DecisionRunner::flush_session_log)
/// failed, or why a [`RunnerConfig`](crate::RunnerConfig) was refused.
///
/// Decision failures are not here: they are in each result's
/// [`DecisionError`](sc_decision_schema::DecisionError).
///
/// # Examples
///
/// ```
/// use sc_agent_log::{LogError, LogKind};
/// use sc_agent_types::ErrorId;
/// use sc_decision_runner::RunnerError;
///
/// let error = RunnerError::SessionLog(LogError::RowsLost { log: LogKind::Session, count: 1, last: None });
/// assert_eq!(error.id(), "runner.session_log");
/// assert_eq!(error.cause_detail().map(|cause| cause.id), Some("log.rows_lost".to_owned()));
/// assert!(error.to_string().starts_with("runner.session_log: "));
/// ```
#[derive(Debug)]
#[non_exhaustive]
pub enum RunnerError {
    /// The session log could not be flushed or lost rows (`log.flush`, `log.rows_lost`),
    /// kept unchanged.
    SessionLog(LogError),
    /// A [`RunnerConfig`](crate::RunnerConfig) out of bounds: `workers` is 0 or over `max`.
    InvalidConfig {
        /// The refused worker count.
        workers: usize,
        /// The most workers allowed ([`MAX_WORKERS`](crate::MAX_WORKERS)).
        max: usize,
    },
}

impl ErrorId for RunnerError {
    fn id(&self) -> &'static str {
        match self {
            Self::SessionLog(_) => "runner.session_log",
            Self::InvalidConfig { .. } => "runner.invalid_config",
        }
    }

    fn message(&self) -> String {
        match self {
            Self::SessionLog(error) => {
                format!(
                    "the session log lost rows or could not be flushed ({})",
                    error.id()
                )
            }
            Self::InvalidConfig { workers, max } => {
                format!("workers must be 1..={max}, got {workers}")
            }
        }
    }

    fn cause_detail(&self) -> Option<ErrorDetail> {
        match self {
            Self::SessionLog(error) => Some(error.detail()),
            Self::InvalidConfig { .. } => None,
        }
    }

    fn recovery(&self) -> Option<&'static str> {
        match self {
            Self::InvalidConfig { .. } => Some("set `workers` to a value from 1 to 64"),
            Self::SessionLog(_) => None,
        }
    }
}

impl fmt::Display for RunnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.id(), self.message())
    }
}

impl Error for RunnerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SessionLog(error) => Some(error),
            Self::InvalidConfig { .. } => None,
        }
    }
}
