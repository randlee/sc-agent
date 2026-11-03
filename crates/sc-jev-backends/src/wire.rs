//! Provider request and reply shapes.
//!
//! The user message is the canonical payload decoded as UTF-8. The injected
//! system prompt is `system` for Haiku and `instructions` for Luna. Neither
//! body is the Jev wire document (ADR-SAG-0019).

use std::collections::BTreeSet;

use sc_agent_types::JevError;
use serde_json::{Map, Value};

use crate::validate::ParsedQuestion;

/// Haiku model id pinned from the jev-check README table.
///
/// Changing it changes the provider body. It is not part of `prompt_sha256`.
pub(crate) const HAIKU_MODEL: &str = "claude-haiku-5-5";

/// Luna model id pinned from the jev-check README table.
pub(crate) const LUNA_MODEL: &str = "gpt-6-luna";

pub(crate) const ANTHROPIC_HOST: &str = "api.anthropic.com";
pub(crate) const OPENAI_HOST: &str = "api.openai.com";
pub(crate) const HAIKU_MESSAGE_PATH: &str = "/v1/messages";
pub(crate) const HAIKU_COUNT_PATH: &str = "/v1/messages/count_tokens";
pub(crate) const LUNA_MESSAGE_PATH: &str = "/v1/responses";
pub(crate) const LUNA_COUNT_PATH: &str = "/v1/responses/input_tokens";

/// Anthropic REST version sent on every Haiku call.
///
/// Pinned here because this crate does not read `check.toml`. A different
/// version is a different wire contract.
pub(crate) const ANTHROPIC_VERSION: &str = "2026-10-21";

/// Haiku completion cap.
///
/// A reply whose `stop_reason` is `max_tokens` is `RESPONSE_INVALID`. The
/// token-count body omits this field.
pub(crate) const HAIKU_MAX_TOKENS: u64 = 16_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderKind {
    Anthropic,
    OpenAi,
}

pub(crate) fn message_body(
    kind: ProviderKind,
    model: &str,
    payload_text: &str,
    system_prompt: &str,
) -> Vec<u8> {
    let user = serde_json::json!([{ "role": "user", "content": payload_text }]);
    let value = match kind {
        ProviderKind::Anthropic => serde_json::json!({
            "model": model,
            "max_tokens": HAIKU_MAX_TOKENS,
            "messages": user,
            "system": system_prompt,
        }),
        ProviderKind::OpenAi => serde_json::json!({
            "model": model,
            "input": user,
            "instructions": system_prompt,
        }),
    };
    encode(&value)
}

pub(crate) fn count_body(kind: ProviderKind, message_body: &[u8]) -> Vec<u8> {
    match kind {
        ProviderKind::OpenAi => message_body.to_vec(),
        ProviderKind::Anthropic => {
            let mut value: Value =
                serde_json::from_slice(message_body).expect("judge body was just encoded as JSON");
            if let Some(object) = value.as_object_mut() {
                object.remove("max_tokens");
            }
            encode(&value)
        }
    }
}

/// Provider headers that do not carry the API key.
///
/// The sending transport adds `x-api-key` or `Authorization` when it reads
/// the sealed credential (ADR-SAG-0020).
pub(crate) fn unsigned_headers(kind: ProviderKind) -> Vec<(String, String)> {
    let mut headers = match kind {
        ProviderKind::Anthropic => {
            vec![("anthropic-version".to_owned(), ANTHROPIC_VERSION.to_owned())]
        }
        ProviderKind::OpenAi => Vec::new(),
    };
    headers.push(("Content-Type".to_owned(), "application/json".to_owned()));
    headers
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VendorReply {
    pub(crate) text: String,
    pub(crate) input_tokens: u64,
    pub(crate) commands: u64,
    pub(crate) stop_reason: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) truncation: Option<String>,
}

pub(crate) fn parse_count(provider: &str, body: &[u8]) -> Result<u64, JevError> {
    let value: Value = serde_json::from_slice(body).map_err(|_| JevError::ResponseInvalid {
        detail: format!("{provider} token count is not JSON"),
    })?;
    let tokens = value.get("input_tokens");
    match tokens.and_then(Value::as_i64) {
        Some(tokens) if tokens >= 0 => Ok(u64::try_from(tokens).unwrap_or(0)),
        _ => Err(JevError::ResponseInvalid {
            detail: format!("{provider} token count has no input_tokens"),
        }),
    }
}

pub(crate) fn parse_vendor(
    kind: ProviderKind,
    provider: &str,
    body: &[u8],
) -> Result<VendorReply, JevError> {
    let value: Value = serde_json::from_slice(body).map_err(|_| JevError::ResponseInvalid {
        detail: format!("{provider} reply is not JSON"),
    })?;
    let object = value.as_object().ok_or_else(|| JevError::ResponseInvalid {
        detail: format!("{provider} reply is not JSON"),
    })?;
    let usage = object.get("usage").and_then(Value::as_object);
    let (text, commands, input_tokens) = match kind {
        ProviderKind::Anthropic => parse_anthropic(object, usage)?,
        ProviderKind::OpenAi => parse_openai(object, usage)?,
    };
    let stop_reason = object
        .get("stop_reason")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let status = object
        .get("status")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let truncation = truncation_reason(provider, object, stop_reason.as_deref(), status.as_deref());
    if text.is_none() && truncation.is_none() {
        return Err(JevError::ResponseInvalid {
            detail: format!("{provider} reply has no text"),
        });
    }
    Ok(VendorReply {
        text: text.unwrap_or_default(),
        input_tokens,
        commands,
        stop_reason,
        status,
        truncation,
    })
}

pub(crate) fn parse_answers(text: &str, questions: &[ParsedQuestion]) -> Result<Value, JevError> {
    let value: Value = serde_json::from_str(text).map_err(|_| JevError::ResponseInvalid {
        detail: "LLM reply is not JSON".into(),
    })?;
    let answers = value
        .get("answers")
        .and_then(Value::as_object)
        .ok_or_else(|| JevError::ResponseInvalid {
            detail: "LLM answer ids differ from the question ids".into(),
        })?;
    let expected: BTreeSet<&str> = questions
        .iter()
        .map(|question| question.id.as_str())
        .collect();
    let actual: BTreeSet<&str> = answers.keys().map(String::as_str).collect();
    if expected != actual {
        return Err(JevError::ResponseInvalid {
            detail: "LLM answer ids differ from the question ids".into(),
        });
    }

    let mut normalized = Map::new();
    for question in questions {
        let item = answers
            .get(&question.id)
            .and_then(Value::as_object)
            .ok_or_else(|| bad_answer(&question.id))?;
        let choice = item.get("choice").and_then(Value::as_str).unwrap_or("");
        if !question.choices.iter().any(|label| label == choice) {
            return Err(bad_answer(&question.id));
        }
        let confidence = item
            .get("confidence")
            .ok_or_else(|| bad_answer(&question.id))?;
        if !is_number(confidence) {
            return Err(not_a_number());
        }
        let probabilities = item
            .get("probabilities")
            .and_then(Value::as_object)
            .ok_or_else(|| bad_answer(&question.id))?;
        let expected_labels: BTreeSet<&str> = question.choices.iter().map(String::as_str).collect();
        let actual_labels: BTreeSet<&str> = probabilities.keys().map(String::as_str).collect();
        if expected_labels != actual_labels {
            return Err(bad_answer(&question.id));
        }
        for label in &question.choices {
            let probability = probabilities
                .get(label)
                .expect("probability key was compared against choices");
            if !is_number(probability) {
                return Err(not_a_number());
            }
        }
        let mut answer = Map::new();
        answer.insert("choice".to_owned(), Value::String(choice.to_owned()));
        answer.insert("confidence".to_owned(), confidence.clone());
        answer.insert(
            "probabilities".to_owned(),
            Value::Object(probabilities.clone()),
        );
        normalized.insert(question.id.clone(), Value::Object(answer));
    }
    Ok(Value::Object(normalized))
}

pub(crate) fn provider_meta(reply: &VendorReply, model: &str, prompt_tokens: u64) -> Value {
    serde_json::json!({
        "model": model,
        "stop_reason": reply.stop_reason,
        "status": reply.status,
        "input_tokens": reply.input_tokens,
        "prompt_tokens": prompt_tokens,
        "commands": reply.commands,
    })
}

fn parse_anthropic(
    object: &Map<String, Value>,
    usage: Option<&Map<String, Value>>,
) -> Result<(Option<String>, u64, u64), JevError> {
    let mut text = String::new();
    let mut commands = 0_u64;
    let mut saw_text = false;
    for block in object
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(block) = block.as_object() else {
            continue;
        };
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(piece) = block.get("text").and_then(Value::as_str) {
                    text.push_str(piece);
                    saw_text = true;
                }
            }
            Some(kind) if kind.ends_with("tool_use") => commands = commands.saturating_add(1),
            _ => {}
        }
    }
    let input_tokens = usage_i64(usage, "input_tokens")
        .saturating_add(usage_i64(usage, "cache_creation_input_tokens"))
        .saturating_add(usage_i64(usage, "cache_read_input_tokens"));
    Ok((saw_text.then_some(text), commands, nonneg(input_tokens)?))
}

fn parse_openai(
    object: &Map<String, Value>,
    usage: Option<&Map<String, Value>>,
) -> Result<(Option<String>, u64, u64), JevError> {
    let mut text = String::new();
    let mut commands = 0_u64;
    let mut saw_text = false;
    for item in object
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(item) = item.as_object() else {
            commands = commands.saturating_add(1);
            continue;
        };
        match item.get("type").and_then(Value::as_str) {
            Some("message") => {
                for block in item
                    .get("content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let Some(block) = block.as_object() else {
                        continue;
                    };
                    if let Some(piece) = block.get("text").and_then(Value::as_str) {
                        text.push_str(piece);
                        saw_text = true;
                    }
                }
            }
            Some("reasoning") => {}
            _ => commands = commands.saturating_add(1),
        }
    }
    Ok((
        saw_text.then_some(text),
        commands,
        nonneg(usage_i64(usage, "input_tokens"))?,
    ))
}

fn truncation_reason(
    provider: &str,
    object: &Map<String, Value>,
    stop_reason: Option<&str>,
    status: Option<&str>,
) -> Option<String> {
    if stop_reason == Some("max_tokens") {
        return Some(format!(
            "{provider} reply truncated at max_tokens (stop_reason=max_tokens)"
        ));
    }
    if let Some(status) =
        status.filter(|status| matches!(*status, "incomplete" | "max_tokens" | "max_output_tokens"))
    {
        return Some(format!(
            "{provider} reply truncated at max_tokens (status={status})"
        ));
    }
    let incomplete = object
        .get("incomplete_details")
        .and_then(|value| value.get("reason"))
        .and_then(Value::as_str);
    if incomplete == Some("max_output_tokens") {
        return Some(format!(
            "{provider} reply truncated at max_tokens (incomplete_details.reason=max_output_tokens)"
        ));
    }
    None
}

fn usage_i64(usage: Option<&Map<String, Value>>, key: &str) -> i64 {
    usage
        .and_then(|object| object.get(key))
        .and_then(Value::as_i64)
        .unwrap_or(0)
}

fn nonneg(value: i64) -> Result<u64, JevError> {
    u64::try_from(value).map_err(|_| JevError::ResponseInvalid {
        detail: "provider input_tokens is negative".into(),
    })
}

fn is_number(value: &Value) -> bool {
    matches!(value, Value::Number(_))
}

fn bad_answer(id: &str) -> JevError {
    JevError::ResponseInvalid {
        detail: format!("LLM answer {id} needs a criterion choice and a probability per criterion"),
    }
}

fn not_a_number() -> JevError {
    JevError::ResponseInvalid {
        detail: "LLM confidence or probability is not a number".into(),
    }
}

fn encode(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("provider JSON values are finite")
}
