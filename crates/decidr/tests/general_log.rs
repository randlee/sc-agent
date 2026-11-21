//! ADR-SAG-0021 row `decidr` CLI: the general log is `<root>/logs/decidr.log.jsonl` with the root
//! from `DECIDR_LOG_ROOT`, a successful send is recorded once, and the general log is separate
//! from the session JSONL file (the empty-variable fallback to `~/.sc-agent` is
//! `sc_agent_log::log_root`'s unit test).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::Duration;
use wait_timeout::ChildExt;

use sc_agent_http::test_server::{Server, ok};
use serde_json::{Value, json};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn run_with_deadline(command: &mut Command, input: Option<&[u8]>) -> Result<Output> {
    let mut child = command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let drain = |mut pipe: Box<dyn Read + Send>| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).map(|_| bytes)
        })
    };
    let stdout = drain(Box::new(child.stdout.take().ok_or("stdout was not piped")?));
    let stderr = drain(Box::new(child.stderr.take().ok_or("stderr was not piped")?));
    let stdin_error = input.and_then(|input| {
        child
            .stdin
            .take()
            .and_then(|mut stdin| stdin.write_all(input).err())
    });
    let status = child.wait_timeout(Duration::from_secs(30))?;
    let timed_out = status.is_none();
    let status = match status {
        Some(status) => status,
        None => {
            child.kill()?;
            child.wait()?
        }
    };
    let output = Output {
        status,
        stdout: stdout.join().map_err(|_| "stdout reader panicked")??,
        stderr: stderr.join().map_err(|_| "stderr reader panicked")??,
    };
    if timed_out {
        return Err(format!("decidr timed out: {output:?}").into());
    }
    if let Some(error) = stdin_error {
        return Err(format!("writing decidr stdin failed: {error}; {output:?}").into());
    }
    Ok(output)
}

fn events(log: &Path) -> Result<Vec<Value>> {
    Ok(std::fs::read_to_string(log)?
        .lines()
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?)
}

fn actions(log: &Path) -> Result<Vec<String>> {
    let mut actions = Vec::new();
    for line in std::fs::read_to_string(log)?.lines() {
        let event: Value = serde_json::from_str(line)?;
        let action = event.get("action").and_then(Value::as_str);
        actions.push(action.unwrap_or_default().to_owned());
    }
    Ok(actions)
}

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

#[test]
fn a_failed_command_is_recorded_in_the_general_log_under_the_variable() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let root = dir.path().join("root");
    let output = run_with_deadline(
        Command::new(env!("CARGO_BIN_EXE_decidr"))
            .args("--project p send --model haiku --agent-role t".split_whitespace())
            .current_dir(dir.path())
            .env("DECIDR_LOG_ROOT", &root),
        None,
    )?;
    assert!(!output.status.success());
    assert_eq!(
        actions(&root.join("logs").join("decidr.log.jsonl"))?,
        ["decidr.error"]
    );
    assert!(!root.join("sessions").exists());
    Ok(())
}

#[test]
fn a_successful_send_is_recorded_once_and_keeps_general_logs_out_of_sessions() -> Result<()> {
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
    let dir = tempfile::tempdir()?;
    let root = dir.path().join("root");
    let output = run_with_deadline(
        Command::new(env!("CARGO_BIN_EXE_decidr"))
            .args("--project p send --model m --runner test --agent-role t".split_whitespace())
            .current_dir(dir.path())
            .env("DECIDR_LOG_ROOT", &root)
            .env("DECIDR_LOCAL_RUNNERS", runner.to_string()),
        Some(br#"{"model":"m","state":"test","questions":{"q":{"type":"noul"}}}"#),
    )?;
    assert!(
        output.status.success(),
        "stdout: {}; stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    server.finish();

    let general_log = root.join("logs").join("decidr.log.jsonl");
    let events = events(&general_log)?;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["action"], "decidr.send");
    assert_eq!(events[0]["fields"]["position"], 0);

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
