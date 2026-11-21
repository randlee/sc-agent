//! ADR-SAG-0021 row `decidr` CLI: the general log is `<root>/logs/decidr.log.jsonl` with the root
//! from `DECIDR_LOG_ROOT`, a successful send is recorded once, and the general log is separate
//! from the session JSONL file (the empty-variable fallback to `~/.sc-agent` is
//! `sc_agent_log::log_root`'s unit test).

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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

// Trace: docs/requirements/REQ-SAG-0003.md:24,26 — use decidr's configured log root.
fn decidr_command(dir: &Path, root: &Path, args: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_decidr"));
    command
        .args(args.split_whitespace())
        .current_dir(dir)
        .env("DECIDR_LOG_ROOT", root);
    command
}

#[test]
fn successful_command_is_logged_outside_sessions() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let root = dir.path().join("root");
    // Finding agt-h-2.cli-general-log-r2-fix remedy: exercise a real successful send.
    let server = Server::start(vec![
        ok(json!({ "models": [{ "name": "m" }] })),
        ok(json!({ "model": "m", "answers": { "q": { "type": "noul", "noul": 0.5 } } })),
    ]);
    let port = server
        .base
        .rsplit(':')
        .next()
        .ok_or("test server address has no port")?
        .parse::<u16>()?;
    let runner = json!({
        "test": { "kind": "generic", "host": "127.0.0.1", "port": port }
    });
    let request = dir.path().join("request.json");
    std::fs::write(
        &request,
        br#"{"model":"m","state":"test","questions":{"q":{"type":"noul"}}}"#,
    )?;
    let output = decidr_command(
        dir.path(),
        &root,
        "--project p send --model m --runner test --agent-role t",
    )
    .env("DECIDR_LOCAL_RUNNERS", runner.to_string())
    .stdin(Stdio::from(File::open(request)?))
    .output()?;
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    server.finish();

    // Trace: docs/requirements/REQ-SAG-0003.md:26; agt-h-2.cli-general-log-r2-fix remedy.
    let general_log = root.join("logs").join("decidr.log.jsonl");
    let records = std::fs::read_to_string(&general_log)?;
    let events = records
        .lines()
        .map(serde_json::from_str)
        .collect::<std::result::Result<Vec<Value>, _>>()?;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["action"], "decidr.send");

    // Trace: docs/requirements/REQ-SAG-0003.md:41; docs/requirements/REQ-SAG-0005.md:23.
    let sessions = root.join("sessions");
    let mut session_files = Vec::new();
    files_under(&sessions, &sessions, &mut session_files)?;
    assert_eq!(session_files.len(), 1);
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
