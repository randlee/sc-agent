//! Pre-send checks for a canonical payload and its injected prompt.
//!
//! Unknown question types and short choice lists refuse before any HTTP call.
//! The byte cap is the assembled judge body (system prompt plus canonical
//! payload), not the payload alone (ADR-SAG-0004, REQ-SAG-0502).

use std::collections::BTreeSet;

use sc_agent_types::JevError;
use serde_json::Value;

/// One choice question taken from the canonical payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedQuestion {
    pub(crate) id: String,
    pub(crate) choices: Vec<String>,
}

/// Pre-send input token ceiling for one judge call (ADR-SAG-0004).
///
/// `validate_for_send` applies [`local_input_tokens`] to the count body and
/// the judge message before any HTTP send. `judge` checks the provider's
/// `input_tokens` against this same ceiling after the count call and before
/// the judge message.
pub(crate) const MAX_INPUT_TOKENS: u64 = 32_000;

/// Conservative token count of an assembled provider body.
///
/// One token per four bytes, rounded up. A body that already passed
/// the 96 000-byte descriptor limit estimates to at most 24_000 tokens, under
/// [`MAX_INPUT_TOKENS`]. The provider count is the check that can still
/// refuse the judge message.
pub(crate) fn local_input_tokens(body: &[u8]) -> u64 {
    let bytes = u64::try_from(body.len()).unwrap_or(u64::MAX);
    bytes.div_ceil(4)
}

/// Refuses `tokens` above [`MAX_INPUT_TOKENS`].
///
/// # Errors
///
/// Returns [`JevError::InputInvalid`] when `tokens` exceeds the ceiling.
pub(crate) fn enforce_input_token_limit(tokens: u64) -> Result<(), JevError> {
    if tokens > MAX_INPUT_TOKENS {
        return Err(JevError::input_invalid(format!(
            "input is {tokens} tokens; limit is {MAX_INPUT_TOKENS} tokens"
        )));
    }
    Ok(())
}

/// Byte and local token checks for the count body and the judge message.
///
/// # Errors
///
/// Returns [`JevError::InputTooLarge`] when either body exceeds
/// `limit_bytes` (the backend descriptor's `request_limit_bytes`; a larger
/// body is never split), and [`JevError::InputInvalid`] when the local token
/// estimate exceeds [`MAX_INPUT_TOKENS`].
pub(crate) fn enforce_count_and_message_limits(
    limit_bytes: u64,
    count_body: &[u8],
    message_body: &[u8],
) -> Result<(), JevError> {
    for body in [count_body, message_body] {
        let actual = u64::try_from(body.len()).unwrap_or(u64::MAX);
        if actual > limit_bytes {
            return Err(JevError::input_too_large(limit_bytes, actual));
        }
        enforce_input_token_limit(local_input_tokens(body))?;
    }
    Ok(())
}

pub(crate) fn validate_questions(bytes: &[u8]) -> Result<Vec<ParsedQuestion>, JevError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        JevError::input_invalid(format!("canonical payload is not JSON: {error}"))
    })?;
    let questions = value
        .get("questions")
        .and_then(Value::as_array)
        .ok_or_else(|| JevError::input_invalid("canonical payload needs a questions array"))?;
    if questions.is_empty() {
        return Err(JevError::input_invalid(
            "a check needs at least one question",
        ));
    }

    let mut unknown = BTreeSet::new();
    for question in questions {
        match question.get("type") {
            None => {}
            Some(Value::String(kind)) if kind == "choice" => {}
            Some(other) => {
                unknown.insert(type_label(other));
            }
        }
    }
    if !unknown.is_empty() {
        let joined = unknown
            .iter()
            .map(|kind| format!("\"{kind}\""))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(JevError::input_invalid(format!(
            "no answer schema for question type [{joined}]; known: [\"choice\"]"
        )));
    }

    let mut parsed = Vec::with_capacity(questions.len());
    let mut seen = BTreeSet::new();
    for question in questions {
        let object = question
            .as_object()
            .ok_or_else(|| JevError::input_invalid("question is not an object"))?;
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| JevError::input_invalid("question is missing id"))?;
        if !seen.insert(id.to_owned()) {
            return Err(JevError::input_invalid(format!(
                "duplicate question id {id}"
            )));
        }
        if object.get("prompt").and_then(Value::as_str).is_none() {
            return Err(JevError::input_invalid(format!(
                "question {id} is missing prompt"
            )));
        }
        let Some(items) = object.get("choices").and_then(Value::as_array) else {
            return Err(JevError::input_invalid(format!(
                "question {id} is not a choice question with two or more criteria"
            )));
        };
        if items.len() < 2 || items.iter().any(|item| item.as_str().is_none()) {
            return Err(JevError::input_invalid(format!(
                "question {id} is not a choice question with two or more criteria"
            )));
        }
        let labels = items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        parsed.push(ParsedQuestion {
            id: id.to_owned(),
            choices: labels,
        });
    }
    Ok(parsed)
}

fn type_label(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_owned)
}
