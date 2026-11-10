//! Shared by the integration tests: real `ApiKey`s (resolved through `resolve_var` over an
//! in-memory environment), backends, requests and canned provider replies.
#![allow(dead_code, reason = "each test binary uses a subset of these helpers")]

use sc_agent_http::HttpResponse;
use sc_agent_secrets::{ApiKey, EnvSource, KeySources, KeyringSource, SecretsError, resolve_var};
use sc_agent_types::{Catalog, ModelChoice, ShortName};
use sc_decision_llm::LlmDecisionBackend;
use sc_decision_schema::{DecisionRequest, SystemOneRequest};
use sc_llm_provider::{LlmClient, LlmProvider};
use serde_json::{Value, json};

/// The test secret; no test output may contain it.
pub const KEY: &str = "sk-decision-llm-test-secret-4567";

struct OneVar;

impl EnvSource for OneVar {
    fn var(&self, name: &str) -> Option<String> {
        (name == "SC_DECISION_LLM_TEST_KEY").then(|| KEY.to_owned())
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
    resolve_var(&"SC_DECISION_LLM_TEST_KEY".parse().expect("var"), &sources)
        .expect("key")
        .key
}

/// Haiku: Anthropic with the catalog's latest Haiku.
pub fn haiku() -> LlmDecisionBackend {
    let model = Catalog::builtin().resolve(&ModelChoice::Latest(ShortName::Haiku));
    LlmDecisionBackend::new(
        LlmClient::new(LlmProvider::Anthropic, key()).expect("client"),
        model,
    )
}

/// Luna: OpenAI with the catalog's latest Luna.
pub fn luna() -> LlmDecisionBackend {
    let model = Catalog::builtin().resolve(&ModelChoice::Latest(ShortName::Luna));
    LlmDecisionBackend::new(
        LlmClient::new(LlmProvider::OpenAi, key()).expect("client"),
        model,
    )
}

/// One question of each Jev type.
pub fn four_questions() -> DecisionRequest {
    let body = SystemOneRequest::from_json(
        &json!({
            "model": "haiku",
            "state": { "ticket": "printer on fire", "floor": 3 },
            "questions": {
                "urgent": { "type": "noul", "instructions": "Is it urgent?" },
                "team": { "type": "choice", "criteria": { "facilities": "building", "it": "computers" } },
                "severity": { "type": "score", "criteria": ["low", "medium", "high"] },
                "where": { "type": "box" }
            }
        })
        .to_string(),
    )
    .expect("request");
    DecisionRequest::new(body, Vec::new(), false).expect("valid")
}

/// One choice question `q` with criteria yes / no.
pub fn choice_request() -> DecisionRequest {
    let body = SystemOneRequest::from_json(
        r#"{"model":"haiku","state":"Is the sky blue?","questions":{"q":{"type":"choice","criteria":{"yes":"it is","no":"it is not"}}}}"#,
    )
    .expect("request");
    DecisionRequest::new(body, Vec::new(), false).expect("valid")
}

/// A reply answering all four questions of [`four_questions`].
pub fn four_answers() -> Value {
    json!({
        "answers": {
            "urgent": { "noul": 0.123456789 },
            "team": {
                "choice": "facilities",
                "confidence": 0.8,
                "probabilities": { "facilities": 0.8, "it": 0.2 }
            },
            "severity": {
                "score": 2,
                "confidence": 0.7,
                "probabilities": { "0": 0.05, "1": 0.25, "2": 0.7 }
            },
            "where": {
                "box": { "top": 0.1, "bottom": 0.9, "left": 0.2, "right": 0.8 },
                "confidence": null
            }
        }
    })
}

/// An Anthropic Messages reply whose text is `text`.
pub fn anthropic_reply(text: &str, stop_reason: &str) -> HttpResponse {
    json_response(
        200,
        &json!({
            "model": "claude-haiku-5-5-20261007",
            "stop_reason": stop_reason,
            "content": [{ "type": "text", "text": text }],
            "usage": { "input_tokens": 120, "output_tokens": 40 }
        }),
    )
}

/// A Chat Completions reply whose text is `text`.
pub fn chat_reply(text: &str, finish_reason: &str) -> HttpResponse {
    json_response(
        200,
        &json!({
            "model": "gpt-6-luna-2026-09-22",
            "choices": [{
                "message": { "role": "assistant", "content": text, "refusal": null },
                "finish_reason": finish_reason
            }],
            "usage": { "prompt_tokens": 110, "completion_tokens": 35 }
        }),
    )
}

pub fn json_response(status: u16, body: &Value) -> HttpResponse {
    HttpResponse::new(status, Vec::new(), body.to_string().into_bytes())
}
