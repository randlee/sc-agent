//! Check/judge body encoding and response parsing.
//!
//! The payload bytes are posted unchanged, with the pinned `model` key spliced
//! in front. This client does not re-canonicalize those bytes for send
//! (ADR-SAG-0005). [`reject_unless_already_canonical`] compares the original
//! bytes to a validation oracle and drops that oracle. [`splice_model`] copies
//! the original payload.

use std::collections::{BTreeMap, BTreeSet};

use sc_agent_types::{
    BackendId, ChoiceId, JevError, JudgeOutcome, JudgeRequest, QuestionAnswer, QuestionId,
};
use serde_json::{Map, Value};

use crate::config::JevClientConfig;

/// Wire body plus the criteria used to validate the reply.
pub(crate) struct PreparedCheck {
    pub(crate) body: Vec<u8>,
    pub(crate) criteria: BTreeMap<String, BTreeSet<String>>,
}

/// Validates `payload` and builds the byte-exact check body.
///
/// The wire body is `payload` with the pinned model spliced in front. This
/// function does not re-encode those bytes for send. The canonical encoding is
/// a validation oracle: it is compared to `payload` and then discarded.
///
/// # Errors
///
/// Returns [`JevError::InputInvalid`] when the payload is not canonical
/// sorted-key compact JSON, is not an object, or has no choice question.
/// Returns [`JevError::InputTooLarge`] when the wire body exceeds
/// `limit_bytes` (the Jev descriptor's `request_limit_bytes`).
pub(crate) fn prepare(
    config: &JevClientConfig,
    limit_bytes: u64,
    payload: &[u8],
) -> Result<PreparedCheck, JevError> {
    let value: Value = serde_json::from_slice(payload)
        .map_err(|_| JevError::input_invalid("Jev payload is not JSON"))?;
    let object = value
        .as_object()
        .ok_or_else(|| JevError::input_invalid("Jev payload must be a JSON object"))?;
    let criteria = question_criteria(&value)?;
    reject_unless_already_canonical(payload, &value)?;
    debug_assert!(
        payload.starts_with(b"{"),
        "canonical encoding of an object starts with '{{'"
    );
    if object.contains_key("model") {
        return Err(JevError::input_invalid(
            "canonical payload must not include model; sc-jev adds the pinned model",
        ));
    }
    if object.keys().any(|key| key.as_str() < "model") {
        return Err(JevError::input_invalid(
            "canonical payload has a key that sorts before model, so the wire body cannot keep those bytes",
        ));
    }
    let body = splice_model(config.model(), payload);
    let actual_bytes = body.len() as u64;
    if actual_bytes > limit_bytes {
        return Err(JevError::input_too_large(limit_bytes, actual_bytes));
    }
    Ok(PreparedCheck { body, criteria })
}

/// Parses a Jev check response into a [`JudgeOutcome`].
///
/// `provider_meta` is that response object with every top-level field kept.
/// The CLI prints it plus `session_id` (ADR-SAG-0014).
///
/// # Errors
///
/// Returns [`JevError::ResponseInvalid`] when the model, answer ids, choice,
/// or probability keys do not match the request, or a confidence or
/// probability is not a finite number in `[0, 1]`.
pub(crate) fn parse_outcome(
    raw: &[u8],
    model: &str,
    criteria: &BTreeMap<String, BTreeSet<String>>,
    request: &JudgeRequest,
) -> Result<JudgeOutcome, JevError> {
    let value: Value = serde_json::from_slice(raw)
        .map_err(|_| JevError::response_invalid("Jev response is not JSON"))?;
    let body = value
        .as_object()
        .ok_or_else(|| JevError::response_invalid("unexpected Jev response model or shape"))?;
    if body.get("model").and_then(Value::as_str) != Some(model) {
        return Err(JevError::response_invalid(
            "unexpected Jev response model or shape",
        ));
    }
    let answers = body
        .get("answers")
        .and_then(Value::as_object)
        .ok_or_else(|| JevError::response_invalid("unexpected Jev response model or shape"))?;
    let expected: BTreeSet<&str> = criteria.keys().map(String::as_str).collect();
    let actual: BTreeSet<&str> = answers.keys().map(String::as_str).collect();
    if expected != actual {
        return Err(JevError::response_invalid(
            "Jev answer ids differ from the question ids",
        ));
    }

    let mut typed = BTreeMap::new();
    for (id, item) in answers {
        let answer = item.as_object().ok_or_else(|| {
            JevError::response_invalid(format!("answer {id} is not one of its criteria"))
        })?;
        let choice = answer
            .get("choice")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                JevError::response_invalid(format!("answer {id} is not one of its criteria"))
            })?;
        let allowed = criteria.get(id).expect("id was taken from this map");
        if !allowed.contains(choice) {
            return Err(JevError::response_invalid(format!(
                "answer {id} is not one of its criteria"
            )));
        }
        let confidence = require_number(answer.get("confidence"))?;
        let probs = answer
            .get("probabilities")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                JevError::response_invalid(format!(
                    "answer {id} probabilities are not exactly the question criteria"
                ))
            })?;
        let got: BTreeSet<&str> = probs.keys().map(String::as_str).collect();
        let want: BTreeSet<&str> = allowed.iter().map(String::as_str).collect();
        if got != want {
            return Err(JevError::response_invalid(format!(
                "answer {id} probabilities are not exactly the question criteria"
            )));
        }
        let mut probabilities = BTreeMap::new();
        for (label, probability) in probs {
            probabilities.insert(response_label(label)?, require_number(Some(probability))?);
        }
        let question = QuestionId::try_from(id.as_str())
            .map_err(|_| JevError::response_invalid(format!("answer id {id:?} is not valid")))?;
        let answer = QuestionAnswer::new(
            response_label(choice)?,
            Some(confidence),
            Some(probabilities),
        )?;
        typed.insert(question, answer);
    }

    Ok(JudgeOutcome::new(BackendId::Jev, request, typed)?
        .with_provider_meta(Value::Object(body.clone())))
}

fn response_label(text: &str) -> Result<ChoiceId, JevError> {
    ChoiceId::try_from(text)
        .map_err(|_| JevError::response_invalid(format!("choice label {text:?} is not valid")))
}

fn question_criteria(payload: &Value) -> Result<BTreeMap<String, BTreeSet<String>>, JevError> {
    let questions = payload
        .get("questions")
        .ok_or_else(|| JevError::input_invalid("a check needs at least one question"))?;
    match questions {
        Value::Object(map) => criteria_from_object(map),
        Value::Array(items) => criteria_from_array(items),
        _ => Err(JevError::input_invalid(
            "questions must be an object or an array",
        )),
    }
}

fn criteria_from_object(
    map: &Map<String, Value>,
) -> Result<BTreeMap<String, BTreeSet<String>>, JevError> {
    if map.is_empty() {
        return Err(JevError::input_invalid(
            "a check needs at least one question",
        ));
    }
    let mut criteria = BTreeMap::new();
    for (id, question) in map {
        let question = question.as_object().ok_or_else(|| {
            JevError::input_invalid(format!(
                "question {id} is not a choice question with two or more criteria"
            ))
        })?;
        let is_choice = question.get("type").and_then(Value::as_str) == Some("choice");
        let fields = question.get("criteria").and_then(Value::as_object);
        if !is_choice || fields.is_none_or(|fields| fields.len() < 2) {
            return Err(JevError::input_invalid(format!(
                "question {id} is not a choice question with two or more criteria"
            )));
        }
        let fields = fields.expect("checked above");
        criteria.insert(id.clone(), fields.keys().cloned().collect());
    }
    Ok(criteria)
}

fn criteria_from_array(items: &[Value]) -> Result<BTreeMap<String, BTreeSet<String>>, JevError> {
    if items.is_empty() {
        return Err(JevError::input_invalid(
            "a check needs at least one question",
        ));
    }
    let mut criteria = BTreeMap::new();
    for item in items {
        let item = item.as_object().ok_or_else(|| {
            JevError::input_invalid("question is not a choice question with two or more criteria")
        })?;
        let id = item.get("id").and_then(Value::as_str).ok_or_else(|| {
            JevError::input_invalid("question is not a choice question with two or more criteria")
        })?;
        let choices = item
            .get("choices")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                JevError::input_invalid(format!(
                    "question {id} is not a choice question with two or more criteria"
                ))
            })?;
        let mut labels = BTreeSet::new();
        for choice in choices {
            let label = choice.as_str().ok_or_else(|| {
                JevError::input_invalid(format!(
                    "question {id} is not a choice question with two or more criteria"
                ))
            })?;
            labels.insert(label.to_owned());
        }
        if labels.len() < 2 {
            return Err(JevError::input_invalid(format!(
                "question {id} is not a choice question with two or more criteria"
            )));
        }
        if criteria.insert(id.to_owned(), labels).is_some() {
            return Err(JevError::input_invalid(format!(
                "question id {id} is duplicated"
            )));
        }
    }
    Ok(criteria)
}

/// Compares `payload` to the canonical encoding of `value` and drops that encoding.
///
/// The oracle checks that the facade already canonicalized the bytes. It is not
/// the wire body. [`splice_model`] copies `payload` unchanged.
fn reject_unless_already_canonical(payload: &[u8], value: &Value) -> Result<(), JevError> {
    let validation_oracle = sc_agent_types::canonical_json_bytes(value)?;
    if validation_oracle.as_slice() != payload {
        return Err(JevError::input_invalid(
            "payload bytes are not canonical sorted-key compact JSON",
        ));
    }
    Ok(())
}

/// `{"model":<model>,` followed by `payload` without its opening brace.
///
/// `prepare` has already checked that `payload` is a canonical object with a
/// `questions` key, so it is never `{}` and always starts with `{`.
fn splice_model(model: &str, payload: &[u8]) -> Vec<u8> {
    debug_assert!(payload.len() > 2 && payload[0] == b'{');
    let model_json = Value::String(model.to_owned()).to_string();
    let mut body = Vec::with_capacity(payload.len() + model_json.len() + 10);
    body.extend_from_slice(br#"{"model":"#);
    body.extend_from_slice(model_json.as_bytes());
    body.push(b',');
    body.extend_from_slice(&payload[1..]);
    body
}

fn require_number(value: Option<&Value>) -> Result<f64, JevError> {
    match value.and_then(Value::as_f64) {
        Some(number) => Ok(number),
        _ => Err(JevError::response_invalid(
            "Jev confidence or probability is not a number",
        )),
    }
}

#[cfg(test)]
mod response_shape {
    use std::collections::{BTreeMap, BTreeSet};

    use super::parse_outcome;

    #[test]
    fn parse_outcome_keeps_other_top_level_fields() {
        let raw = br#"{"model":"jev-test","answers":{"q":{"choice":"no","confidence":0.9,"probabilities":{"no":0.9,"yes":0.1}}},"usage":{"input_tokens":318,"output_tokens":31},"stop_reason":"end_turn"}"#;
        let mut labels = BTreeSet::new();
        labels.insert("yes".to_owned());
        labels.insert("no".to_owned());
        let mut criteria = BTreeMap::new();
        criteria.insert("q".to_owned(), labels);
        let payload: sc_agent_types::CanonicalJudgePayload =
            serde_json::from_value(serde_json::json!({
                "questions": [{"id": "q", "prompt": "p", "choices": ["no", "yes"]}],
                "state": []
            }))
            .expect("payload");
        let request = sc_agent_types::JudgeRequest::new(
            std::sync::Arc::new(sc_agent_types::PayloadBundle::encode(&payload).expect("bundle")),
            None,
        );
        let outcome = parse_outcome(raw, "jev-test", &criteria, &request).expect("parse");
        let meta = outcome.provider_meta().expect("meta");
        assert_eq!(meta["stop_reason"], "end_turn");
        assert_eq!(meta["model"], "jev-test");
        assert_eq!(meta["usage"]["input_tokens"], 318);
        assert_eq!(meta["answers"]["q"]["choice"], "no");
        assert_eq!(outcome.answers()["q"].choice().as_str(), "no");
    }
}
