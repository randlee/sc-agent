#[path = "../src/cli.rs"]
mod cli;
use clap::Parser;
use serde_json::{Value, json};
use std::{
    io::Write,
    net::TcpListener,
    process::{Command, Stdio},
};

#[test]
fn req_sag_0004_0702_cli_accepts_project_role_timeout_and_batch_workers()
-> Result<(), Box<dyn std::error::Error>> {
    let parsed = cli::Cli::try_parse_from([
        "decidr",
        "--project",
        "phase-b",
        "send",
        "--agent-role",
        "rust-developer",
        "--timeout-secs",
        "10",
        "--batch",
        "--workers",
        "2",
    ])?;
    assert_eq!(parsed.project.as_deref(), Some("phase-b"));
    match parsed.command {
        cli::Command::Send {
            agent_role,
            timeout_secs,
            batch,
            workers,
            ..
        } => {
            assert_eq!(agent_role, "rust-developer");
            assert_eq!(timeout_secs, 10.0);
            assert!(batch);
            assert_eq!(workers, Some(2));
        }
        _ => return Err("send command not selected".into()),
    }
    Ok(())
}

#[test]
fn req_sag_0702_cli_timeout_reaches_a_nonresponding_loopback_runner()
-> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let runner = std::thread::spawn(move || -> Result<usize, std::io::Error> {
        let mut unanswered = Vec::new();
        for _ in 0..5 {
            unanswered.push(listener.accept()?.0);
        }
        let count = unanswered.len();
        drop(unanswered);
        Ok(count)
    });

    let root = std::env::temp_dir().join(format!("decidr-timeout-{}", std::process::id()));
    std::fs::create_dir_all(&root)?;
    let config = json!({
        "project": "phase-i",
        "laya": {"host": "127.0.0.1", "port": address.port(), "model": "configured-local"}
    });
    std::fs::write(root.join("decidr.json"), config.to_string())?;
    let mut child = Command::new(env!("CARGO_BIN_EXE_decidr"))
        .args([
            "--config-dir",
            root.to_str().ok_or("temporary path is not UTF-8")?,
            "send",
            "--model",
            "laya",
            "--agent-role",
            "contract",
            "--timeout-secs",
            "0.001",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("DECIDR_LOG_ROOT", &root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or("child stdin unavailable")?
        .write_all(br#"{"state":"timeout","questions":{"q":{"type":"noul"}}}"#)?;
    let output = child.wait_with_output()?;
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert!(
        !output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(report["error"]["id"], "http.network");
    assert_eq!(report["error"]["attempts"], 5);
    assert_eq!(runner.join().map_err(|_| "runner panicked")??, 5);
    std::fs::remove_dir_all(root)?;
    Ok(())
}
