//! Shared by the integration tests: a real `ApiKey` (resolved through `resolve_var` over an
//! in-memory environment, no process environment or keyring) and canned replies.
#![allow(dead_code, reason = "each test binary uses a subset of these helpers")]

use sc_agent_http::HttpResponse;
use sc_agent_secrets::{ApiKey, EnvSource, KeySources, KeyringSource, SecretsError, resolve_var};
use serde_json::Value;

/// The test secret; no test output may contain it.
pub const KEY: &str = "sk-llm-provider-test-secret-0123";

struct OneVar;

impl EnvSource for OneVar {
    fn var(&self, name: &str) -> Option<String> {
        (name == "SC_LLM_PROVIDER_TEST_KEY").then(|| KEY.to_owned())
    }
}

struct NoKeyring;

impl KeyringSource for NoKeyring {
    fn password(&self, _: &str, _: &str) -> Result<Option<String>, SecretsError> {
        Ok(None)
    }
}

pub fn key() -> ApiKey {
    let sources = KeySources {
        env: &OneVar,
        repo_root: None,
        keyring: &NoKeyring,
    };
    resolve_var(&"SC_LLM_PROVIDER_TEST_KEY".parse().expect("var"), &sources)
        .expect("key")
        .key
}

pub fn response(status: u16, headers: Vec<(&str, &str)>, body: Value) -> HttpResponse {
    text_response(
        status,
        headers,
        &serde_json::to_string(&body).expect("json"),
    )
}

pub fn text_response(status: u16, headers: Vec<(&str, &str)>, body: &str) -> HttpResponse {
    HttpResponse::new(
        status,
        headers
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect(),
        body.as_bytes().to_vec(),
    )
}

/// An Anthropic Messages reply with one text block.
pub fn haiku_message(text: &str, stop_reason: &str, input_tokens: i64) -> HttpResponse {
    response(
        200,
        Vec::new(),
        serde_json::json!({
            "model": "claude-haiku-5-5-20261007",
            "stop_reason": stop_reason,
            "content": [{ "type": "text", "text": text }],
            "usage": {
                "input_tokens": input_tokens,
                "cache_read_input_tokens": 2,
                "output_tokens": 9
            }
        }),
    )
}

/// A Chat Completions reply with one choice.
pub fn chat_message(text: &str, finish_reason: &str) -> HttpResponse {
    response(
        200,
        Vec::new(),
        serde_json::json!({
            "model": "gpt-6-luna-2026-09-22",
            "choices": [{
                "index": 0,
                "message": { "role": "assistant", "content": text, "refusal": null },
                "finish_reason": finish_reason
            }],
            "usage": { "prompt_tokens": 21, "completion_tokens": 5 }
        }),
    )
}
