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

/// Single-request byte cap shared with the Jev check client.
///
/// A larger judge body is `INPUT_TOO_LARGE` and is never split.
pub(crate) const MAX_REQUEST_BYTES: u64 = 96_000;

pub(crate) fn validate_questions(bytes: &[u8]) -> Result<Vec<ParsedQuestion>, JevError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| JevError::InputInvalid {
        detail: format!("canonical payload is not JSON: {error}"),
    })?;
    let questions = value
        .get("questions")
        .and_then(Value::as_array)
        .ok_or_else(|| JevError::InputInvalid {
            detail: "canonical payload needs a questions array".into(),
        })?;
    if questions.is_empty() {
        return Err(JevError::InputInvalid {
            detail: "a check needs at least one question".into(),
        });
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
        return Err(JevError::InputInvalid {
            detail: format!("no answer schema for question type [{joined}]; known: [\"choice\"]"),
        });
    }

    let mut parsed = Vec::with_capacity(questions.len());
    let mut seen = BTreeSet::new();
    for question in questions {
        let object = question.as_object().ok_or_else(|| JevError::InputInvalid {
            detail: "question is not an object".into(),
        })?;
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| JevError::InputInvalid {
                detail: "question is missing id".into(),
            })?;
        if !seen.insert(id.to_owned()) {
            return Err(JevError::InputInvalid {
                detail: format!("duplicate question id {id}"),
            });
        }
        if object.get("prompt").and_then(Value::as_str).is_none() {
            return Err(JevError::InputInvalid {
                detail: format!("question {id} is missing prompt"),
            });
        }
        let Some(items) = object.get("choices").and_then(Value::as_array) else {
            return Err(JevError::InputInvalid {
                detail: format!("question {id} is not a choice question with two or more criteria"),
            });
        };
        if items.len() < 2 || items.iter().any(|item| item.as_str().is_none()) {
            return Err(JevError::InputInvalid {
                detail: format!("question {id} is not a choice question with two or more criteria"),
            });
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

pub(crate) fn too_large(actual_bytes: u64) -> JevError {
    JevError::InputTooLarge {
        limit_bytes: MAX_REQUEST_BYTES,
        actual_bytes,
    }
}
