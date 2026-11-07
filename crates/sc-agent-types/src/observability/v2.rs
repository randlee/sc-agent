//! Injectable session log aligned with `sc-observability` v2.
//!
//! Libraries take [`SharedSessionLog`]. Composition roots adapt a real v2
//! logger to this trait. This module depends on `sc-observability-types` only.

use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use crate::error::JevError;
use crate::ids::{BackendId, SessionId, Sha256Digest};

/// Rejection returned by a [`SessionLog`] implementation.
#[doc(inline)]
pub use sc_observability_types::v2::EventError;

/// One prompt or judge send.
///
/// Fields are correlation metadata only. Secrets never appear here
/// (NFR-SAG-0002).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEvent {
    /// Session the send belongs to.
    pub session_id: SessionId,
    /// Backend that received the payload.
    pub backend: BackendId,
    /// SHA-256 of the canonical payload bytes.
    pub prompt_sha256: Sha256Digest,
    /// Which send this is (RBP-F011).
    pub operation: LogOperation,
}

/// The send a [`LogEvent`] records.
///
/// Log adapters write [`Self::as_str`] (`judge` or `ask`), the same strings
/// the field carried before it was typed, so the JSONL output is unchanged.
/// Every value is a valid `sc-observability` action name, so adapters need
/// no fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogOperation {
    /// A `run_check` judge send.
    Judge,
    /// A session `ask` send.
    Ask,
}

impl LogOperation {
    /// Every operation, in declaration order.
    pub const ALL: [Self; 2] = [Self::Judge, Self::Ask];

    /// The wire and action-name spelling: `judge` or `ask`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Judge => "judge",
            Self::Ask => "ask",
        }
    }
}

impl fmt::Display for LogOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for LogOperation {
    type Err = JevError;

    /// Parses the [`Self::as_str`] spelling.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::observability::v2::LogOperation;
    ///
    /// assert_eq!("ask".parse::<LogOperation>().unwrap(), LogOperation::Ask);
    /// assert_eq!("Judge".parse::<LogOperation>().unwrap_err().code(), "INPUT_INVALID");
    /// ```
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|operation| operation.as_str() == text)
            .ok_or_else(|| JevError::InputInvalid {
                detail: format!("log operation must be judge or ask, not {text:?}"),
            })
    }
}

/// Best-effort session logger.
///
/// Implementations return promptly and do not panic. The trait is
/// intentionally open (not sealed): composition roots (`sc-ai`, `sc-jev-py`)
/// adapt their own `sc-observability` logger to it and tests inject doubles,
/// so third-party implementations are expected.
pub trait SessionLog: Send + Sync {
    /// Records `event`.
    ///
    /// # Errors
    ///
    /// Returns [`EventError`] when the receiver rejects the event. Callers
    /// treat that as best effort.
    fn try_log(&self, event: LogEvent) -> Result<(), EventError>;
}

/// Shared session log injected into `sc-jev-check`.
pub type SharedSessionLog = Arc<dyn SessionLog>;

/// Discards every event.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopSessionLog;

impl SessionLog for NoopSessionLog {
    fn try_log(&self, _event: LogEvent) -> Result<(), EventError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn event() -> LogEvent {
        LogEvent {
            session_id: SessionId::from_uuid(Uuid::nil()),
            backend: BackendId::Jev,
            prompt_sha256: Sha256Digest::of(b"payload"),
            operation: LogOperation::Judge,
        }
    }

    #[test]
    fn noop_accepts_events() {
        NoopSessionLog.try_log(event()).expect("noop accepts");
    }

    #[test]
    fn log_event_fields_carry_no_secrets() {
        let event = event();
        let LogEvent {
            session_id,
            backend,
            prompt_sha256,
            operation,
        } = event;
        let rendered = format!("{session_id} {backend} {prompt_sha256} {operation}");
        let lower = rendered.to_ascii_lowercase();
        for forbidden in ["secret", "api_key", "password", "authorization", "bearer"] {
            assert!(!lower.contains(forbidden), "{forbidden} in {rendered}");
        }
        assert_eq!(operation, LogOperation::Judge);
    }

    #[test]
    fn log_operation_round_trips_its_wire_spelling() {
        for operation in LogOperation::ALL {
            assert_eq!(operation.to_string(), operation.as_str());
            assert_eq!(operation.as_str().parse::<LogOperation>(), Ok(operation));
        }
        assert_eq!(LogOperation::Judge.as_str(), "judge");
        assert_eq!(LogOperation::Ask.as_str(), "ask");
        assert!("event".parse::<LogOperation>().is_err());
    }
}
