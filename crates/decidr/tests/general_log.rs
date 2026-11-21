//! ADR-SAG-0021 row `decidr` CLI: the general log is `<root>/logs/decidr.log.jsonl` with the root
//! from `DECIDR_LOG_ROOT`, a successful send is recorded once, and the general log is separate
//! from the session JSONL file (the empty-variable fallback to `~/.sc-agent` is
//! `sc_agent_log::log_root`'s unit test).

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use sc_agent_http::test_server::{Server, ok};
use serde_json::{Value, json};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const CHILD_DEADLINE: Duration = Duration::from_secs(30);
const CHILD_POLL_INTERVAL: Duration = Duration::from_millis(10);

enum ChildWait {
    Exited(io::Result<ExitStatus>),
    TimedOut(io::Result<ExitStatus>),
}

fn run_with_deadline(command: &mut Command, stdin_data: Option<&[u8]>) -> Result<Output> {
    command
        .stdin(if stdin_data.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let mut stdout = child.stdout.take().ok_or("stdout was not piped")?;
    let mut stderr = child.stderr.take().ok_or("stderr was not piped")?;
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });

    // Save stdin errors, including BrokenPipe, until the child's output is drained.
    let stdin_result = if let Some(data) = stdin_data {
        let result = match child.stdin.take() {
            Some(mut stdin) => stdin.write_all(data),
            None => Err(io::Error::other("stdin was not piped")),
        };
        result.err()
    } else {
        None
    };

    let (kill_tx, kill_rx) = mpsc::channel();
    let (wait_tx, wait_rx) = mpsc::channel();
    let waiter = thread::spawn(move || {
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let _ = wait_tx.send(ChildWait::Exited(Ok(status)));
                    return;
                }
                Ok(None) => {}
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = wait_tx.send(ChildWait::Exited(Err(error)));
                    return;
                }
            }
            match kill_rx.recv_timeout(CHILD_POLL_INTERVAL) {
                Ok(()) => {
                    let _ = child.kill();
                    let status = child.wait();
                    let _ = wait_tx.send(ChildWait::TimedOut(status));
                    return;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let _ = child.kill();
                    let status = child.wait();
                    let _ = wait_tx.send(ChildWait::TimedOut(status));
                    return;
                }
            }
        }
    });

    let wait = match wait_rx.recv_timeout(CHILD_DEADLINE) {
        Ok(wait) => wait,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            let _ = kill_tx.send(());
            wait_rx.recv()?
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            return Err(io::Error::other("decidr child waiter disconnected").into());
        }
    };
    drop(kill_tx);
    waiter
        .join()
        .map_err(|_| io::Error::other("decidr child waiter panicked"))?;

    let stdout = stdout_reader
        .join()
        .map_err(|_| io::Error::other("stdout reader panicked"))??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| io::Error::other("stderr reader panicked"))??;
    let captured = format!(
        "stdout: {}; stderr: {}",
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(&stderr)
    );
    if let Some(error) = stdin_result {
        return Err(io::Error::new(
            error.kind(),
            format!("writing decidr stdin failed: {error}; {captured}"),
        )
        .into());
    }
    match wait {
        ChildWait::Exited(Ok(status)) => Ok(Output {
            status,
            stdout,
            stderr,
        }),
        ChildWait::Exited(Err(error)) => Err(io::Error::new(
            error.kind(),
            format!("waiting for decidr failed: {error}; {captured}"),
        )
        .into()),
        ChildWait::TimedOut(status) => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!(
                "decidr exceeded the {:?} failure-only deadline ({status:?}); {captured}",
                CHILD_DEADLINE
            ),
        )
        .into()),
    }
}

fn events(log: &Path) -> Result<Vec<Value>> {
    let mut events = Vec::new();
    for line in std::fs::read_to_string(log)?.lines() {
        events.push(serde_json::from_str(line)?);
    }
    Ok(events)
}

fn actions(log: &Path) -> Result<Vec<String>> {
    Ok(events(log)?
        .iter()
        .map(|event| {
            event
                .get("action")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        })
        .collect())
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
    let mut command = Command::new(env!("CARGO_BIN_EXE_decidr"));
    command
        .args([
            "--project",
            "p",
            "send",
            "--model",
            "haiku",
            "--agent-role",
            "t",
        ])
        .current_dir(dir.path())
        .env("DECIDR_LOG_ROOT", &root);
    let output = run_with_deadline(&mut command, None)?;
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
    let mut command = Command::new(env!("CARGO_BIN_EXE_decidr"));
    command
        .args([
            "--project",
            "p",
            "send",
            "--model",
            "m",
            "--runner",
            "test",
            "--agent-role",
            "t",
        ])
        .current_dir(dir.path())
        .env("DECIDR_LOG_ROOT", &root)
        .env("DECIDR_LOCAL_RUNNERS", runner.to_string());
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
    let seen = server.finish();
    assert_eq!(
        seen.iter()
            .map(|request| request.path.as_str())
            .collect::<Vec<_>>(),
        ["/v1/models", "/v1/systemone"]
    );

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
