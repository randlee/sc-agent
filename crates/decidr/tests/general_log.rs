//! ADR-SAG-0021 row `decidr` CLI: the general log is `<root>/logs/decidr.log.jsonl` with the root
//! from `DECIDR_LOG_ROOT`, and no general log is written under `sessions/` (the empty-variable
//! fallback to `~/.sc-agent` is `sc_agent_log::log_root`'s unit test). The run has a project but
//! no request on stdin, so it fails before any session, key lookup or network call.

use std::path::Path;
use std::process::{Command, Stdio};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn actions(log: &Path) -> Result<Vec<String>> {
    let mut actions = Vec::new();
    for line in std::fs::read_to_string(log)?.lines() {
        let event: serde_json::Value = serde_json::from_str(line)?;
        let action = event.get("action").and_then(serde_json::Value::as_str);
        actions.push(action.unwrap_or_default().to_owned());
    }
    Ok(actions)
}

#[test]
fn a_failed_command_is_recorded_in_the_general_log_under_the_variable() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let root = dir.path().join("root");
    let output = Command::new(env!("CARGO_BIN_EXE_decidr"))
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
        .env("DECIDR_LOG_ROOT", &root)
        .stdin(Stdio::null())
        .output()?;
    assert!(!output.status.success());
    assert_eq!(
        actions(&root.join("logs").join("decidr.log.jsonl"))?,
        ["decidr.error"]
    );
    assert!(!root.join("sessions").exists());
    Ok(())
}
