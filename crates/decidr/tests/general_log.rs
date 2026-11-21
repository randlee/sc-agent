//! The shared CLI/wheel send path writes one general record separately from session rows.
//! Controlled Config inputs keep this test independent of the host's global configuration.

use std::path::{Path, PathBuf};

use sc_agent_http::test_server::{Server, ok};
use serde_json::{Value, json};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// Trace: docs/requirements/REQ-SAG-0003.md:26,41; docs/requirements/REQ-SAG-0005.md:23.
fn files_under(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            files_under(root, &path, files)?;
        } else {
            files.push(path.strip_prefix(root)?.to_path_buf());
        }
    }
    Ok(())
}

fn events(log: &Path) -> Result<Vec<Value>> {
    let mut events = Vec::new();
    for line in std::fs::read_to_string(log)?.lines() {
        events.push(serde_json::from_str(line)?);
    }
    Ok(events)
}

#[test]
fn shared_send_is_logged_outside_sessions() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let root = dir.path().join("root");
    // Exercise the same send path as CLI/Python with controlled configuration.
    let server = Server::start(vec![
        ok(json!({ "loaded": ["m"] })),
        ok(json!({ "model": "m", "answers": { "q": { "type": "noul", "noul": 0.5 } } })),
    ]);
    let port = server
        .base
        .rsplit(':')
        .next()
        .ok_or("test server address has no port")?
        .parse::<u16>()?;
    let config: sc_agent_config::Config = serde_json::from_value(json!({ "local_runners": {
        "test": { "kind": "laya", "model": "m", "host": "127.0.0.1", "port": port }
    }}))?;
    let logger = sc_agent_log::general("decidr", &root)?;
    let session = sc_agent_log::Session::open(&root, sc_agent_log::repo_id(dir.path()), "p", "t")?;
    let request: sc_decision_schema::DecisionRequest = serde_json::from_value(
        json!({"model":"m","state":"test","questions":{"q":{"type":"noul"}}}),
    )?;
    let digests = sc_decision_runner::digests(&request);
    let mut results = Vec::new();
    decidr::send(
        vec![request],
        "m",
        Some("test"),
        None,
        &config,
        None,
        None,
        Some(dir.path()),
        Some(session),
        None,
        &logger,
        &sc_agent_http::Cancel::default(),
        &mut |o| {
            assert_eq!(o.sends, 1);
            assert!(
                o.result
                    .as_ref()
                    .is_ok_and(|r| r.extra.get("digests") == Some(&json!(digests)))
            );
            results.push(o.result.is_ok());
        },
    )?;
    assert_eq!(results, [true]);
    logger.flush()?;
    let seen = server.finish();
    assert_eq!(seen[1].json()["model"], "m");

    // Trace: docs/requirements/REQ-SAG-0003.md:26; agt-h-2.cli-general-log-r2-fix remedy.
    let general_log = root.join("logs").join("decidr.log.jsonl");
    let events = events(&general_log)?;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["action"], "decidr.send");

    // Trace: docs/requirements/REQ-SAG-0003.md:41; docs/requirements/REQ-SAG-0005.md:23.
    let sessions = root.join("sessions");
    let mut session_files = Vec::new();
    files_under(&sessions, &sessions, &mut session_files)?;
    assert_eq!(session_files.len(), 1);
    let row: Value = serde_json::from_str(
        std::fs::read_to_string(sessions.join(&session_files[0]))?
            .lines()
            .next()
            .ok_or("session row missing")?,
    )?;
    assert_eq!(row["fields"]["digests"], json!(digests));
    assert_eq!(
        session_files[0].file_name().and_then(|name| name.to_str()),
        Some("sessions.jsonl")
    );
    assert!(
        !session_files
            .iter()
            .any(|path| path.ends_with("decidr.log.jsonl"))
    );
    Ok(())
}
