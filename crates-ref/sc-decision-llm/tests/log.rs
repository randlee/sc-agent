//! AC 18 (ANC-007): one decode writes `llm.usage`; a mismatch adds `llm.schema_mismatch`.

// Clippy allows unwrap, expect, panic and indexing only in test code (clippy.toml);
// the whole file, helpers included, is test code.
#![cfg(test)]

mod common;

use common::{KEY, anthropic_reply, choice_request, haiku};
use sc_agent_log::LogContext;
use sc_agent_log::testing::MemoryLog;
use sc_agent_types::SessionId;
use sc_decision_schema::Backend;
use serde_json::json;

const STATE: &str = "Is the sky blue?";

#[test]
fn usage_and_schema_mismatch_records() {
    let log = MemoryLog::new().expect("memory log");
    let backend = haiku().with_log(log.general());
    assert!(format!("{backend:?}").contains("<set>"));
    let request = choice_request();
    let session_id = SessionId::new();
    let ctx = LogContext::request(session_id, Some(5));

    let good = json!({ "answers": { "q": {
        "choice": "yes", "confidence": 0.9, "probabilities": { "yes": 0.9, "no": 0.1 }
    } } })
    .to_string();
    backend
        .decode(anthropic_reply(&good, "end_turn"), &request, &ctx)
        .expect("matching reply");
    let bad = json!({ "answers": { "q": { "choice": "yes", "confidence": 0.9 } } }).to_string();
    let error = backend
        .decode(anthropic_reply(&bad, "end_turn"), &request, &ctx)
        .expect_err("missing probabilities");
    assert!(matches!(
        error,
        sc_decision_schema::DecisionError::SchemaMismatch { .. }
    ));

    let records = log.records().expect("records");
    let actions: Vec<&str> = records.iter().map(|r| r.action.as_str()).collect();
    assert_eq!(actions, ["llm.usage", "llm.usage", "llm.schema_mismatch"]);
    let mismatch = &records[2];
    assert_eq!(mismatch.target, "sc_decision_llm");
    assert_eq!(mismatch.fields["provider"], "anthropic");
    assert_eq!(mismatch.fields["model"], "claude-haiku-5-5");
    assert_eq!(mismatch.fields["error"], "decision_llm.answer_invalid");
    assert_eq!(mismatch.fields["question"], "q");
    for record in &records {
        assert_eq!(record.fields["session_id"], session_id.to_string());
        assert_eq!(record.fields["position"], json!(5));
    }
    assert_eq!(records[0].target, "sc_llm_provider");
    let rendered = format!("{records:?}");
    for hidden in [KEY, STATE, "You answer decision questions", "probabilities"] {
        assert!(!rendered.contains(hidden), "{hidden} in {rendered}");
    }
}
