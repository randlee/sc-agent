//! Session log port (REQ-SAG-0107, REQ-SAG-0120/0121 amended, REQ-SAG-0610).

use std::sync::{Arc, Mutex};

use sc_agent_types::observability::v2::{
    LogEvent, LogRejected, NoopSessionLog, SessionLog, SharedSessionLog,
};
use sc_agent_types::{BackendId, Operation, SessionId, Sha256Digest};

fn event(operation: Operation) -> LogEvent {
    LogEvent::new(
        SessionId::from_uuid(uuid::Uuid::nil()),
        BackendId::Jev,
        Sha256Digest::of(b"payload"),
        operation,
    )
}

#[test]
fn test_log_rejected_display() {
    let rejected = LogRejected::new("queue full");
    assert_eq!(rejected.reason(), "queue full");
    assert_eq!(
        rejected.to_string(),
        "session log rejected the event: queue full"
    );
    let boxed: Box<dyn std::error::Error> = Box::new(rejected);
    assert!(boxed.source().is_none());
}

#[test]
fn test_noop_session_log_accepts_events() {
    let log: SharedSessionLog = Arc::new(NoopSessionLog);
    for operation in Operation::ALL.iter().copied() {
        log.emit(event(operation)).expect("noop accepts");
    }
}

/// The port is open: a third-party implementation records events.
#[test]
fn test_session_log_is_an_open_port() {
    #[derive(Default)]
    struct Recording(Mutex<Vec<LogEvent>>);

    impl SessionLog for Recording {
        fn emit(&self, event: LogEvent) -> Result<(), LogRejected> {
            self.0.lock().expect("lock").push(event);
            Ok(())
        }
    }

    let log = Arc::new(Recording::default());
    let shared: SharedSessionLog = Arc::clone(&log) as SharedSessionLog;
    shared.emit(event(Operation::Ask)).expect("emit");
    let seen = log.0.lock().expect("lock").clone();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].operation(), Operation::Ask);
}

/// `LogEvent.operation` is the typed `Operation` (REQ-SAG-0610).
#[test]
fn test_log_event_operation_is_typed_and_carries_no_secret() {
    for operation in Operation::ALL.iter().copied() {
        assert_eq!(
            operation.as_str().parse::<Operation>().expect("parse"),
            operation
        );
        let event = event(operation);
        assert_eq!(event.operation(), operation);
        let rendered = format!(
            "{} {} {} {}",
            event.session_id(),
            event.backend(),
            event.prompt_sha256(),
            event.operation()
        )
        .to_ascii_lowercase();
        for forbidden in ["secret", "api_key", "password", "authorization", "bearer"] {
            assert!(!rendered.contains(forbidden), "{forbidden}");
        }
    }
    assert_eq!(Operation::Judge.as_str(), "judge");
    assert_eq!(Operation::Ask.as_str(), "ask");
}
