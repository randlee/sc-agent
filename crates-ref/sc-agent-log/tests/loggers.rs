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
    // Session logs live under logs/ too: only the general log file must be absent.
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
