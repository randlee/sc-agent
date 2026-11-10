//! The general log: one file per CLI, generic rows, LogContext (REQ-SAG-0003, RSH-009).

mod common;

use common::{events, keys, payload};
use sc_agent_log::testing::MemoryLog;
use sc_agent_log::{GeneralLogger, Level, LogContext, LogError};
use sc_agent_types::SessionId;
use serde_json::json;
use tempfile::TempDir;

/// A8-03: open_default(log_root, service) writes <log_root>/logs/<service>.log.jsonl, one file
/// per CLI name.
#[test]
fn general_file_per_cli() {
    let root = TempDir::new().expect("temp");
    for service in ["decide", "sc-agent-py"] {
        let log = GeneralLogger::open_default(root.path(), service).expect(service);
        let row = payload(&[("cli", json!(service))]);
        log.write(
            Level::Info,
            "sc_agent_log",
            "test.row",
            LogContext::NONE,
            row,
        )
        .expect("row");
        log.flush().expect("flush");
    }
    for service in ["decide", "sc-agent-py"] {
        let rows = events(&root.path().join(format!("logs/{service}.log.jsonl"))).expect("rows");
        assert_eq!(rows.len(), 1, "{service}");
        assert_eq!(rows[0].fields["cli"], service);
        assert_eq!(rows[0].service.as_str(), service);
    }
}

/// A8-13: the general logger takes a generic serde_json::Map row and writes it unchanged.
#[test]
fn general_write_generic() {
    let root = TempDir::new().expect("temp");
    let log = GeneralLogger::open_default(root.path(), "decide").expect("logger");
    let row = payload(&[
        ("status", json!(429)),
        ("retry_after_seconds", json!(1.25)),
        ("provider", json!("anthropic")),
        ("nested", json!({"a": [1, 2]})),
    ]);
    log.write(
        Level::Warn,
        "sc_agent_http",
        "http.busy",
        LogContext::NONE,
        row.clone(),
    )
    .expect("row");
    log.flush().expect("flush");
    let rows = events(&root.path().join("logs/decide.log.jsonl")).expect("rows");
    assert_eq!(rows[0].fields, row);
    assert_eq!(rows[0].level, Level::Warn);
    assert_eq!(rows[0].target.as_str(), "sc_agent_http");
    assert_eq!(rows[0].action.as_str(), "http.busy");
}

/// A8-20: a LogContext's session_id is the correlation id and first field, its position the
/// field position; NONE adds neither; two contexts give different session ids; a payload
/// session_id key is log.reserved_field.
#[test]
fn context_on_general_events() {
    let memory = MemoryLog::new().expect("memory");
    let log = memory.general();
    let (first, second) = (SessionId::new(), SessionId::new());
    let row = || payload(&[("elapsed_ms", json!(7))]);
    for ctx in [
        LogContext::request(first, Some(0)),
        LogContext::request(second, None),
        LogContext::NONE,
    ] {
        log.write(Level::Info, "sc_agent_http", "http.response", ctx, row())
            .expect("row");
    }
    let events = memory.events().expect("events");
    assert_eq!(events.len(), 3);

    assert_eq!(
        keys(&events[0].fields),
        ["session_id", "position", "elapsed_ms"]
    );
    assert_eq!(events[0].fields["session_id"], first.to_string());
    assert_eq!(events[0].fields["position"], 0);
    assert_eq!(
        events[0]
            .correlation_id
            .as_ref()
            .map(|id| id.as_str().to_owned()),
        Some(first.to_string())
    );

    assert_eq!(keys(&events[1].fields), ["session_id", "elapsed_ms"]);
    assert_eq!(events[1].fields["session_id"], second.to_string());
    assert_ne!(events[0].correlation_id, events[1].correlation_id);

    assert_eq!(keys(&events[2].fields), ["elapsed_ms"]);
    assert_eq!(events[2].correlation_id, None);

    let error = log
        .write(
            Level::Info,
            "sc_agent_http",
            "http.response",
            LogContext::request(first, None),
            payload(&[("session_id", json!("spoof"))]),
        )
        .expect_err("reserved");
    assert!(matches!(error, LogError::ReservedField { ref key } if key == "session_id"));
    assert!(matches!(
        log.flush(),
        Err(LogError::RowsLost { count: 1, .. })
    ));
}
