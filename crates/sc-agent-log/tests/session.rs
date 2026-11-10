//! Session log layout, appends, self-describing rows, scale (REQ-SAG-0005, NFR-SAG-0001).

mod common;

use std::collections::HashSet;

use common::{events, identity, keys, payload};
use sc_agent_log::{Level, SessionLogger};
use sc_agent_types::SessionId;
use serde_json::{Map, json};
use tempfile::TempDir;

/// A8-09: rows for github.com/randlee/sc-agent, rust-developer, phase-b are in
/// <log_root>/logs/github.com/randlee/sc-agent/rust-developer/phase-b/sessions.jsonl.
#[test]
fn rows_under_layout() {
    let root = TempDir::new().expect("temp");
    let log =
        SessionLogger::open_default(root.path(), identity().expect("identity")).expect("logger");
    log.write(SessionId::new(), Map::new()).expect("row");
    log.flush().expect("flush");
    let file = root
        .path()
        .join("logs")
        .join("github.com")
        .join("randlee")
        .join("sc-agent")
        .join("rust-developer")
        .join("phase-b")
        .join("sessions.jsonl");
    assert_eq!(log.path(), Some(file.as_path()));
    assert_eq!(events(&file).expect("rows").len(), 1);
}

/// A8-09: every later run and every concurrent logger with that identity appends to the same
/// file, and it is the only file in the directory.
#[test]
fn runs_append_one_file() {
    const PER_LOGGER: usize = 100;
    let root = TempDir::new().expect("temp");
    for _run in 0..2 {
        let log = SessionLogger::open_default(root.path(), identity().expect("identity"))
            .expect("logger");
        log.write(SessionId::new(), Map::new()).expect("row");
        log.flush().expect("flush");
    }
    std::thread::scope(|scope| {
        for _ in 0..3 {
            scope.spawn(|| {
                let log = SessionLogger::open_default(root.path(), identity().expect("identity"))
                    .expect("logger");
                for _ in 0..PER_LOGGER {
                    log.write(SessionId::new(), Map::new()).expect("row");
                }
                log.flush().expect("flush");
            });
        }
    });
    let dir = identity().expect("identity").dir(root.path());
    let names: Vec<_> = std::fs::read_dir(&dir)
        .expect("dir")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    assert_eq!(names, [SessionLogger::FILE]);
    assert_eq!(
        events(&dir.join(SessionLogger::FILE)).expect("rows").len(),
        2 + 3 * PER_LOGGER
    );
}

/// A8-10: every row's fields start with session_id, repo, project, agent_role, then the payload
/// in order; session_id parses as a ULID and is the event's correlation id.
#[test]
fn rows_self_describe() {
    let root = TempDir::new().expect("temp");
    let log =
        SessionLogger::open_default(root.path(), identity().expect("identity")).expect("logger");
    let session_id = SessionId::new();
    let row = payload(&[("zeta", json!(1)), ("alpha", json!({"nested": true}))]);
    log.write(session_id, row).expect("row");
    log.flush().expect("flush");
    let rows = events(log.path().expect("path")).expect("rows");
    let event = &rows[0];
    assert_eq!(
        keys(&event.fields),
        [
            "session_id",
            "repo",
            "project",
            "agent_role",
            "zeta",
            "alpha"
        ]
    );
    assert_eq!(event.fields["repo"], "github.com/randlee/sc-agent");
    assert_eq!(event.fields["project"], "phase-b");
    assert_eq!(event.fields["agent_role"], "rust-developer");
    let text = event.fields["session_id"].as_str().expect("text");
    let parsed: SessionId = text.parse().expect("a ULID");
    assert_eq!(parsed, session_id);
    assert_eq!(
        event.correlation_id.as_ref().map(|id| id.as_str()),
        Some(text)
    );
    assert_eq!(event.level, Level::Info);
    assert_eq!(event.target.as_str(), SessionLogger::TARGET);
    assert_eq!(event.action.as_str(), SessionLogger::ACTION);
}

/// A8-11 (NFR-SAG-0001): 40,000 writes with distinct session ids give 40,000 rows in one file.
#[test]
fn forty_thousand_sessions() {
    const SESSIONS: usize = 40_000;
    let root = TempDir::new().expect("temp");
    let log =
        SessionLogger::open_default(root.path(), identity().expect("identity")).expect("logger");
    for _ in 0..SESSIONS {
        log.write(SessionId::new(), Map::new()).expect("row");
    }
    log.flush().expect("flush");
    let text = std::fs::read_to_string(log.path().expect("path")).expect("file");
    let ids: HashSet<&str> = text
        .lines()
        .map(|line| {
            let start = line.find("\"session_id\":\"").expect("session_id") + 14;
            &line[start..start + 26]
        })
        .collect();
    assert_eq!(text.lines().count(), SESSIONS);
    assert_eq!(ids.len(), SESSIONS, "distinct session ids");
}
