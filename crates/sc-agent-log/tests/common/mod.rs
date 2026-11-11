//! Helpers shared by the sc-agent-log integration tests.
#![allow(
    dead_code,
    reason = "each test binary uses a different subset of these helpers"
)]

use std::error::Error;
use std::path::Path;

use sc_agent_log::{LogError, Repo, SessionIdentity};
use sc_observability::v2::LogEvent;
use serde_json::{Map, Value};

/// The identity of A8-09: github.com/randlee/sc-agent, rust-developer, phase-b.
pub fn identity() -> Result<SessionIdentity, LogError> {
    let repo = Repo::from_origin_url("https://github.com/randlee/sc-agent.git")?;
    SessionIdentity::new(repo, "phase-b", "rust-developer")
}

/// A payload of `pairs`, in order.
pub fn payload(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.clone()))
        .collect()
}

/// Every row of a JSONL log file, parsed back into sc-observability events.
pub fn events(path: &Path) -> Result<Vec<LogEvent>, Box<dyn Error>> {
    let text = std::fs::read_to_string(path)?;
    let rows = text
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// The field names of `fields`, in order.
pub fn keys(fields: &Map<String, Value>) -> Vec<&str> {
    fields.keys().map(String::as_str).collect()
}

/// Makes `dir` read-only; returns false (so the test skips) when the process can still write
/// there, as root can.
#[cfg(unix)]
pub fn make_unwritable(dir: &Path) -> std::io::Result<bool> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555))?;
    let probe = dir.join(".probe");
    let writable = std::fs::write(&probe, b"x").is_ok();
    let _ = std::fs::remove_file(probe);
    Ok(!writable)
}

/// Makes `dir` writable again.
#[cfg(unix)]
pub fn make_writable(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755))
}
