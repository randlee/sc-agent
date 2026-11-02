//! Injectable session log aligned with `sc-observability` v2.
//!
//! Libraries take [`SharedSessionLog`]. Composition roots adapt a real v2
//! logger to this trait. This module depends on `sc-observability-types` only.

use std::sync::Arc;

use uuid::Uuid;

use crate::ids::{BackendId, Sha256Digest};

/// Rejection returned by a [`SessionLog`] implementation.
#[doc(inline)]
pub use sc_observability_types::v2::EventError;

/// One prompt or judge send.
///
/// Fields are correlation metadata only. Secrets never appear here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEvent {
    /// Session the send belongs to.
    pub session_id: Uuid,
    /// Backend that received the payload.
    pub backend: BackendId,
    /// SHA-256 of the canonical payload bytes.
    pub prompt_sha256: Sha256Digest,
    /// Operation name, such as `judge` or `ask`.
    pub operation: String,
}

/// Best-effort session logger.
///
/// Implementations return promptly and do not panic.
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

#[cfg(any(test, feature = "test-util"))]
mod doubles {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};

    use sc_observability_types::{ErrorContext, Remediation, error_codes};

    use super::{EventError, LogEvent, SessionLog};

    /// Records events in call order.
    #[derive(Debug, Default)]
    pub struct RecordingSessionLog {
        events: Mutex<Vec<LogEvent>>,
    }

    impl RecordingSessionLog {
        /// Creates an empty recorder.
        #[must_use]
        pub fn new() -> Self {
            Self::default()
        }

        /// Returns a snapshot of recorded events.
        #[must_use]
        pub fn events(&self) -> Vec<LogEvent> {
            self.lock().clone()
        }

        fn lock(&self) -> std::sync::MutexGuard<'_, Vec<LogEvent>> {
            self.events
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        }
    }

    impl SessionLog for RecordingSessionLog {
        fn try_log(&self, event: LogEvent) -> Result<(), EventError> {
            self.lock().push(event);
            Ok(())
        }
    }

    /// Rejects every event and counts the attempts.
    #[derive(Debug, Default)]
    pub struct RejectingSessionLog {
        attempts: AtomicU64,
    }

    impl RejectingSessionLog {
        /// Creates a rejecting log with zero attempts.
        #[must_use]
        pub fn new() -> Self {
            Self::default()
        }

        /// Returns how many `try_log` calls were rejected.
        #[must_use]
        pub fn attempts(&self) -> u64 {
            self.attempts.load(Ordering::Relaxed)
        }
    }

    impl SessionLog for RejectingSessionLog {
        fn try_log(&self, _event: LogEvent) -> Result<(), EventError> {
            self.attempts.fetch_add(1, Ordering::Relaxed);
            Err(EventError::Routing {
                context: Box::new(ErrorContext::new(
                    error_codes::DIAGNOSTIC_INVALID,
                    "test session log rejected the event",
                    Remediation::recoverable(
                        "Use RecordingSessionLog when the test should accept events.",
                        [] as [&str; 0],
                    ),
                )),
            })
        }
    }
}

#[cfg(any(test, feature = "test-util"))]
pub use doubles::{RecordingSessionLog, RejectingSessionLog};

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn event() -> LogEvent {
        LogEvent {
            session_id: Uuid::nil(),
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
    fn recording_and_rejecting_doubles() {
        let recording = RecordingSessionLog::new();
        recording.try_log(event()).expect("record");
        assert_eq!(recording.events().len(), 1);
        let rejecting = RejectingSessionLog::new();
        assert!(rejecting.try_log(event()).is_err());
        assert_eq!(rejecting.attempts(), 1);
    }
}
