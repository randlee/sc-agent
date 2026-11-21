//! ADR-SAG-0021 row `decidr` CLI: the general log is `<root>/logs/decidr.log.jsonl` with the root
//! from `DECIDR_LOG_ROOT`, a successful send is recorded once, and the general log is separate
//! from the session JSONL file (the empty-variable fallback to `~/.sc-agent` is
//! `sc_agent_log::log_root`'s unit test).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::Duration;

use sc_agent_http::test_server::{Server, ok};
use serde_json::{Value, json};
use wait_timeout::ChildExt;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// Finding agt-h-2.qa1-f32-r1-fix: bound the CLI wait and drain both output pipes concurrently.
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
    let stdin_writer = input.and_then(|input| {
        child.stdin.take().map(|mut stdin| {
            let input = input.to_vec();
            thread::spawn(move || stdin.write_all(&input))
        })
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
    let stdin_error = match stdin_writer {
        Some(writer) => writer.join().map_err(|_| "stdin writer panicked")?.err(),
        None => None,
    };
    if timed_out {
        return Err(format!("decidr timed out: {output:?}").into());
    }
    if let Some(error) = stdin_error {
        return Err(format!("writing decidr stdin failed: {error}; {output:?}").into());
    }
    Ok(output)
}

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

fn events(log: &Path) -> Result<Vec<Value>> {
    let mut events = Vec::new();
    for line in std::fs::read_to_string(log)?.lines() {
        events.push(serde_json::from_str(line)?);
    }
    Ok(events)
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
    let mut command = decidr_command(
        dir.path(),
        &root,
        "--project p send --model m --runner test --agent-role t",
    );
    command.env("DECIDR_LOCAL_RUNNERS", runner.to_string());
    let output = run_with_deadline(
        &mut command,
        Some(br#"{"model":"m","state":"test","questions":{"q":{"type":"noul"}}}"#),
    )?;
    assert!(
        output.status.success(),
        "stdout: {}; stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    server.finish();

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
