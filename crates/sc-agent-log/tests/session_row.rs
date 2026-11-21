//! A session row carries the repo, project and agent role. This file uses only what the
//! starting code has, so it also runs against the starting code's src (rows unchanged).

use std::path::Path;

use sc_agent_log::Session;
use serde_json::{Map, Value, json};

fn lines(path: &Path) -> Vec<Value> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

fn fields(pairs: Value) -> Map<String, Value> {
    pairs.as_object().cloned().unwrap_or_default()
}

#[test]
fn session_row_carries_repo_project_and_role() {
    let root = tempfile::tempdir().expect("tempdir");
    let session =
        Session::open(root.path(), "github.com/o/r".into(), "proj", "role").expect("session");
    session.row(fields(json!({ "position": 0 })));
    drop(session);
    let rows = lines(
        &root
            .path()
            .join("sessions")
            .join("github.com")
            .join("o")
            .join("r")
            .join("role")
            .join("proj")
            .join("sessions.jsonl"),
    );
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0]["target"], "sc_agent.session");
    assert_eq!(rows[0]["action"], "session.row");
    let f = &rows[0]["fields"];
    assert_eq!(
        (&f["repo"], &f["project"], &f["agent_role"], &f["position"]),
        (
            &json!("github.com/o/r"),
            &json!("proj"),
            &json!("role"),
            &json!(0)
        )
    );
}
