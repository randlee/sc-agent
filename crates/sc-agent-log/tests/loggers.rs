//! Logger injection (REQ-SAG-0008) and the in-memory test log (feature `testing`).

mod common;

use common::{events, identity, keys, payload};
use sc_agent_log::testing::{MemoryLog, MemoryRecord};
use sc_agent_log::{Level, LogContext, LogError, Loggers, SessionLogger};
use sc_agent_types::SessionId;
use serde_json::{Map, json};
use tempfile::TempDir;

fn write_both(loggers: &Loggers) -> Result<(), LogError> {
    loggers.general.write(
        Level::Info,
        "sc_agent_log",
        "test.general",
        LogContext::NONE,
        Map::new(),
    )?;
    loggers.session.write(SessionId::new(), Map::new())?;
    loggers.general.flush()?;
    loggers.session.flush()
}

/// A8-12: no injection: both default files are written.
#[test]
fn defaults_write_both() {
    let root = TempDir::new().expect("temp");
    let loggers = Loggers::open(
        root.path(),
        "decidr",
        identity().expect("identity"),
        None,
        None,
    )
    .expect("open");
    write_both(&loggers).expect("both rows");
    assert_eq!(
        events(&root.path().join("logs").join("decidr.log.jsonl"))
            .expect("rows")
            .len(),
        1
    );
    assert_eq!(
        events(
            &identity()
                .expect("identity")
                .dir(root.path())
                .join(SessionLogger::FILE)
        )
        .expect("rows")
        .len(),
        1
    );
}

/// A8-12: an injected session logger gets the rows; the general log is the default file.
#[test]
fn injected_session_only() {
    let root = TempDir::new().expect("temp");
    let memory = MemoryLog::new().expect("memory");
    let loggers = Loggers::open(
        root.path(),
        "decidr",
        identity().expect("identity"),
        None,
        Some(memory.logger()),
    )
    .expect("open");
    write_both(&loggers).expect("both rows");
    let records = memory.records().expect("records");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].action, SessionLogger::ACTION);
    assert_eq!(loggers.session.path(), None);
    assert_eq!(
        events(&root.path().join("logs").join("decidr.log.jsonl"))
            .expect("rows")
            .len(),
        1
    );
    assert!(
        !identity().expect("identity").dir(root.path()).exists(),
        "no default session file"
    );
}

/// A8-12: an injected general logger gets the general log; rows go to the default session file.
#[test]
fn injected_general_only() {
    let root = TempDir::new().expect("temp");
    let memory = MemoryLog::new().expect("memory");
    let loggers = Loggers::open(
        root.path(),
        "decidr",
        identity().expect("identity"),
        Some(memory.logger()),
        None,
    )
    .expect("open");
    write_both(&loggers).expect("both rows");
    let records = memory.records().expect("records");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].action, "test.general");
    assert_eq!(
        events(
            &identity()
                .expect("identity")
                .dir(root.path())
                .join(SessionLogger::FILE)
        )
        .expect("rows")
        .len(),
        1
    );
    // An injected general logger creates no default general file.
    let general_files = std::fs::read_dir(root.path().join("logs"))
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "jsonl"))
                .count()
        })
        .unwrap_or(0);
    assert_eq!(general_files, 0, "no default general file");
}

/// A8-12: dropping loggers built on an injected logger never shuts it down.
#[test]
fn injected_logger_not_shut_down() {
    let root = TempDir::new().expect("temp");
    let memory = MemoryLog::new().expect("memory");
    let loggers = Loggers::open(
        root.path(),
        "decidr",
        identity().expect("identity"),
        Some(memory.logger()),
        Some(memory.logger()),
    )
    .expect("open");
    write_both(&loggers).expect("both rows");
    drop(loggers);
    let general = memory.general();
    general
        .write(
            Level::Info,
            "sc_agent_log",
            "after.drop",
            LogContext::NONE,
            Map::new(),
        )
        .expect("the injected logger still runs");
    general.flush().expect("flush");
    let records = memory.records().expect("records");
    assert_eq!(records.len(), 3);
    assert_eq!(records[2].action, "after.drop");
}

/// A8-19: GeneralLogger and SessionLogger from MemoryLog write no file; records() returns each
/// event's level, target, action and fields in write order.
#[test]
fn memory_log_records() {
    let memory = MemoryLog::new().expect("memory");
    let general = memory.general();
    let session = memory.session(identity().expect("identity"));
    let session_id = SessionId::new();
    general
        .write(
            Level::Warn,
            "sc_agent_http",
            "http.busy",
            LogContext::NONE,
            payload(&[("status", json!(503))]),
        )
        .expect("general");
    session
        .write(session_id, payload(&[("decision", json!("yes"))]))
        .expect("session");
    general
        .write(
            Level::Error,
            "sc_decision_runner",
            "runner.failed",
            LogContext::NONE,
            Map::new(),
        )
        .expect("general");
    let records = memory.records().expect("records");
    assert_eq!(
        records[0],
        MemoryRecord {
            level: Level::Warn,
            target: "sc_agent_http".into(),
            action: "http.busy".into(),
            fields: payload(&[("status", json!(503))]),
        }
    );
    assert_eq!(records[1].level, Level::Info);
    assert_eq!(records[1].target, SessionLogger::TARGET);
    assert_eq!(
        keys(&records[1].fields),
        ["session_id", "repo", "project", "agent_role", "decision"]
    );
    assert_eq!(records[2].action, "runner.failed");
    assert_eq!(records.len(), 3);

    // The only sink is the in-memory one: no file sink exists to write anywhere.
    let health = memory.logger().health();
    let sinks: Vec<_> = health
        .sink_statuses
        .iter()
        .map(|sink| sink.name.as_str().to_owned())
        .collect();
    assert_eq!(sinks, ["memory"]);
}

#[test]
fn general_open_with_selects_injection_or_default_without_a_session() {
    let root = tempfile::tempdir().expect("root");
    let memory = MemoryLog::new().expect("memory");
    let injected = sc_agent_log::Injected::Rust(memory.logger());
    let log = sc_agent_log::GeneralLogger::open_with(root.path(), "decidr", Some(injected.clone()))
        .expect("injected");
    log.write(
        Level::Info,
        "test",
        "injected.general",
        LogContext::NONE,
        serde_json::Map::new(),
    )
    .expect("write");
    log.close().expect("close");
    assert_eq!(
        memory.records().expect("records")[0].action,
        "injected.general"
    );
    assert!(!root.path().join("logs").exists());
    let default =
        sc_agent_log::GeneralLogger::open_with(root.path(), "decidr", None).expect("default");
    default
        .write(
            Level::Info,
            "test",
            "default.general",
            LogContext::NONE,
            serde_json::Map::new(),
        )
        .expect("write");
    default.close().expect("close");
    assert!(
        std::fs::read_to_string(root.path().join("logs/decidr.log.jsonl"))
            .expect("rows")
            .contains("default.general")
    );
    assert!(!root.path().join("sessions").exists());
}

#[test]
fn session_open_with_selects_injection_or_default_without_a_general_log() {
    let root = tempfile::tempdir().expect("root");
    let memory = MemoryLog::new().expect("memory");
    let id = identity().expect("identity");
    let injected = sc_agent_log::Injected::Rust(memory.logger());
    let log = sc_agent_log::SessionLogger::open_with(root.path(), id.clone(), Some(injected))
        .expect("injected");
    log.write(SessionId::new(), serde_json::Map::new())
        .expect("write");
    log.close().expect("close");
    assert_eq!(memory.records().expect("records")[0].action, "session.row");
    assert!(!root.path().join("sessions").exists());
    let default =
        sc_agent_log::SessionLogger::open_with(root.path(), id.clone(), None).expect("default");
    let path = default.path().expect("path").to_owned();
    default
        .write(SessionId::new(), serde_json::Map::new())
        .expect("write");
    default.close().expect("close");
    assert!(path.starts_with(id.dir(root.path())));
    assert_eq!(
        std::fs::read_to_string(path).expect("rows").lines().count(),
        1
    );
    assert!(!root.path().join("logs").exists());
}
