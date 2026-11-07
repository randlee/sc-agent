//! Injectable session log aligned with `sc-observability` v2.
//!
//! Libraries take [`SharedSessionLog`]. Composition roots adapt a real v2
//! logger to this trait. This module depends on `sc-observability-types` only.

use std::sync::Arc;

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
    /// Operation name, such as `judge` or `ask`.
    pub operation: String,
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
            operation: "judge".into(),
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
        assert_eq!(operation, "judge");
    }
}
