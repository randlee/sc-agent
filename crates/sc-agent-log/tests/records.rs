//! The general Logger writes records with the target, action and fields they were given.

use std::path::Path;

use sc_agent_log::{general, record};
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
fn general_logger_writes_records_to_the_service_log() {
    let root = tempfile::tempdir().expect("tempdir");
    let logger = general("t", root.path()).expect("logger");
    record(
        logger.as_ref(),
        "sc_agent.test",
        "test.done",
        fields(json!({ "k": 1 })),
    );
    logger.flush().expect("flush");
    let rows = lines(&root.path().join("logs").join("t.log.jsonl"));
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0]["target"], "sc_agent.test");
    assert_eq!(rows[0]["action"], "test.done");
    assert_eq!(rows[0]["fields"]["k"], 1);
}
