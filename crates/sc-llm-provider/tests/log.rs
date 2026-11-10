//! AC 18 (ANC-007): one `llm.usage` record per decoded reply, carrying the LogContext.
// Clippy allows unwrap, expect, panic and indexing only in test code (clippy.toml);
// the whole file, helpers included, is test code.
#![cfg(test)]

mod common;

use common::{KEY, haiku_message};
use sc_agent_log::LogContext;
use sc_agent_log::testing::MemoryLog;
use sc_agent_types::SessionId;
use sc_llm_provider::{LlmClient, LlmProvider, LlmRequest, Message, Output, Part, Role};
use serde_json::json;

const PROMPT: &str = "prompt-text-that-must-not-be-logged";
const REPLY: &str = "reply-text-that-must-not-be-logged";

#[test]
fn decode_writes_one_usage_record_with_the_context() {
    let log = MemoryLog::new().expect("memory log");
    let client = LlmClient::new(LlmProvider::Anthropic, common::key())
        .expect("client")
        .with_log(log.general());
    assert!(format!("{client:?}").contains("<set>"));
    let request = LlmRequest {
        model: "claude-haiku-5-5".parse().expect("model"),
        system: Some(PROMPT.to_owned()),
        messages: vec![Message {
            role: Role::User,
            content: vec![Part::Text(PROMPT.to_owned())],
        }],
        max_tokens: 64,
        output: Output::Text,
    };
    let session_id = SessionId::new();
    client
        .decode(
            &request,
            haiku_message(REPLY, "end_turn", 8),
            LogContext::request(session_id, Some(3)),
        )
        .expect("decode");

    let records = log.records().expect("records");
    let usage: Vec<_> = records
        .iter()
        .filter(|record| record.action == "llm.usage")
        .collect();
    assert_eq!(usage.len(), 1, "{records:?}");
    let record = usage[0];
    assert_eq!(record.target, "sc_llm_provider");
    assert_eq!(record.fields["provider"], "anthropic");
    assert_eq!(record.fields["model"], "claude-haiku-5-5-20261007");
    assert_eq!(record.fields["input_tokens"], json!(8));
    assert_eq!(record.fields["output_tokens"], json!(9));
    assert_eq!(record.fields["session_id"], session_id.to_string());
    assert_eq!(record.fields["position"], json!(3));
    let rendered = format!("{records:?}");
    for hidden in [KEY, PROMPT, REPLY] {
        assert!(!rendered.contains(hidden), "{hidden} in {rendered}");
    }
}

#[test]
fn truncated_reply_still_logs_usage() {
    let log = MemoryLog::new().expect("memory log");
    let client = LlmClient::new(LlmProvider::Anthropic, common::key())
        .expect("client")
        .with_log(log.general());
    let request = LlmRequest {
        model: "claude-haiku-5-5".parse().expect("model"),
        system: None,
        messages: Vec::new(),
        max_tokens: 1,
        output: Output::Text,
    };
    let error = client
        .decode(
            &request,
            haiku_message(REPLY, "max_tokens", 4),
            LogContext::NONE,
        )
        .expect_err("truncated");
    assert!(matches!(error, sc_llm_provider::LlmError::Truncated { .. }));
    let records = log.records().expect("records");
    assert_eq!(
        records.iter().filter(|r| r.action == "llm.usage").count(),
        1
    );
}
