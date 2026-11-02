//! Check/judge body encoding and response parsing.
//!
//! The canonical payload bytes are posted unchanged, with the pinned `model`
//! key spliced in front. The client does not re-canonicalize the payload.

use std::collections::{BTreeMap, BTreeSet};

use sc_agent_types::{BackendId, JevError, JudgeOutcome, canonical_json_bytes};
use serde_json::{Map, Value};

use crate::api::JevClientConfig;

/// Wire body plus the criteria used to validate the reply.
pub(crate) struct PreparedCheck {
    pub(crate) body: Vec<u8>,
    pub(crate) criteria: BTreeMap<String, BTreeSet<String>>,
}

/// Validates `payload` and builds the byte-exact check body.
///
/// # Errors
///
/// Returns [`JevError::InputInvalid`] when the payload is not a canonical
/// object with at least one choice question, and [`JevError::InputTooLarge`]
/// when the wire body exceeds `config.max_request_bytes`.
pub(crate) fn prepare(config: &JevClientConfig, payload: &[u8]) -> Result<PreparedCheck, JevError> {
    let value: Value = serde_json::from_slice(payload).map_err(|_| JevError::InputInvalid {
        detail: "Jev payload is not JSON".to_owned(),
    })?;
    let object = value.as_object().ok_or_else(|| JevError::InputInvalid {
        detail: "Jev payload must be a JSON object".to_owned(),
    })?;
    let criteria = question_criteria(&value)?;
    let canonical = canonical_json_bytes(&value)?;
    if canonical.as_slice() != payload {
        return Err(JevError::InputInvalid {
            detail: "payload bytes are not canonical sorted-key compact JSON".to_owned(),
        });
    }
    if object.contains_key("model") {
        return Err(JevError::InputInvalid {
            detail: "canonical payload must not include model; sc-jev adds the pinned model"
                .to_owned(),
        });
    }
    if object.keys().any(|key| key.as_str() < "model") {
        return Err(JevError::InputInvalid {
            detail: "canonical payload has a key that sorts before model, so the wire body cannot keep those bytes"
                .to_owned(),
        });
    }
    let body = splice_model(config.model(), payload)?;
    let actual_bytes = u64::try_from(body.len()).map_err(|_| JevError::InputTooLarge {
        limit_bytes: config.max_request_bytes(),
        actual_bytes: u64::MAX,
    })?;
    if actual_bytes > config.max_request_bytes() {
        return Err(JevError::InputTooLarge {
            limit_bytes: config.max_request_bytes(),
            actual_bytes,
        });
    }
    Ok(PreparedCheck { body, criteria })
}

/// Parses a Jev check response into a [`JudgeOutcome`].
///
/// # Errors
///
/// Returns [`JevError::ResponseInvalid`] when the model, answer ids, choice,
/// or probability keys do not match the request.
pub(crate) fn parse_outcome(
    raw: &[u8],
    model: &str,
    criteria: &BTreeMap<String, BTreeSet<String>>,
) -> Result<JudgeOutcome, JevError> {
    let value: Value = serde_json::from_slice(raw).map_err(|_| JevError::ResponseInvalid {
        detail: "Jev response is not JSON".to_owned(),
    })?;
    let body = value.as_object().ok_or_else(|| JevError::ResponseInvalid {
        detail: "unexpected Jev response model or shape".to_owned(),
    })?;
    if body.get("model").and_then(Value::as_str) != Some(model) {
        return Err(JevError::ResponseInvalid {
            detail: "unexpected Jev response model or shape".to_owned(),
        });
    }
    let answers = body
        .get("answers")
        .and_then(Value::as_object)
        .ok_or_else(|| JevError::ResponseInvalid {
            detail: "unexpected Jev response model or shape".to_owned(),
        })?;
    let expected: BTreeSet<&str> = criteria.keys().map(String::as_str).collect();
    let actual: BTreeSet<&str> = answers.keys().map(String::as_str).collect();
    if expected != actual {
        return Err(JevError::ResponseInvalid {
            detail: "Jev answer ids differ from the question ids".to_owned(),
        });
    }

    let mut probabilities = Map::new();
    for (id, item) in answers {
        let answer = item.as_object().ok_or_else(|| JevError::ResponseInvalid {
            detail: format!("answer {id} is not one of its criteria"),
        })?;
        let choice = answer
            .get("choice")
            .and_then(Value::as_str)
            .ok_or_else(|| JevError::ResponseInvalid {
                detail: format!("answer {id} is not one of its criteria"),
            })?;
        let allowed = criteria.get(id).expect("id was taken from this map");
        if !allowed.contains(choice) {
            return Err(JevError::ResponseInvalid {
                detail: format!("answer {id} is not one of its criteria"),
            });
        }
        require_number(answer.get("confidence"))?;
        let probs = answer
            .get("probabilities")
            .and_then(Value::as_object)
            .ok_or_else(|| JevError::ResponseInvalid {
                detail: format!("answer {id} probabilities are not exactly the question criteria"),
            })?;
        let got: BTreeSet<&str> = probs.keys().map(String::as_str).collect();
        let want: BTreeSet<&str> = allowed.iter().map(String::as_str).collect();
        if got != want {
            return Err(JevError::ResponseInvalid {
                detail: format!("answer {id} probabilities are not exactly the question criteria"),
            });
        }
        for probability in probs.values() {
            require_number(Some(probability))?;
        }
        probabilities.insert(id.clone(), Value::Object(probs.clone()));
    }

    let mut provider_meta = Map::new();
    provider_meta.insert("model".to_owned(), Value::String(model.to_owned()));
    if let Some(usage) = body.get("usage").filter(|usage| usage.is_object()) {
        provider_meta.insert("usage".to_owned(), usage.clone());
    }

    Ok(JudgeOutcome {
        answers: Value::Object(answers.clone()),
        backend: BackendId::Jev,
        probabilities: Some(Value::Object(probabilities)),
        provider_meta: Some(Value::Object(provider_meta)),
    })
}

fn question_criteria(payload: &Value) -> Result<BTreeMap<String, BTreeSet<String>>, JevError> {
    let questions = payload
        .get("questions")
        .ok_or_else(|| JevError::InputInvalid {
            detail: "a check needs at least one question".to_owned(),
        })?;
    match questions {
        Value::Object(map) => criteria_from_object(map),
        Value::Array(items) => criteria_from_array(items),
        _ => Err(JevError::InputInvalid {
            detail: "questions must be an object or an array".to_owned(),
        }),
    }
}

fn criteria_from_object(
    map: &Map<String, Value>,
) -> Result<BTreeMap<String, BTreeSet<String>>, JevError> {
    if map.is_empty() {
        return Err(JevError::InputInvalid {
            detail: "a check needs at least one question".to_owned(),
        });
    }
    let mut criteria = BTreeMap::new();
    for (id, question) in map {
        let question = question.as_object().ok_or_else(|| JevError::InputInvalid {
            detail: format!("question {id} is not a choice question with two or more criteria"),
        })?;
        let is_choice = question.get("type").and_then(Value::as_str) == Some("choice");
        let fields = question.get("criteria").and_then(Value::as_object);
        if !is_choice || fields.is_none_or(|fields| fields.len() < 2) {
            return Err(JevError::InputInvalid {
                detail: format!("question {id} is not a choice question with two or more criteria"),
            });
        }
        let fields = fields.expect("checked above");
        criteria.insert(id.clone(), fields.keys().cloned().collect());
    }
    Ok(criteria)
}

fn criteria_from_array(items: &[Value]) -> Result<BTreeMap<String, BTreeSet<String>>, JevError> {
    if items.is_empty() {
        return Err(JevError::InputInvalid {
            detail: "a check needs at least one question".to_owned(),
        });
    }
    let mut criteria = BTreeMap::new();
    for item in items {
        let item = item.as_object().ok_or_else(|| JevError::InputInvalid {
            detail: "question is not a choice question with two or more criteria".to_owned(),
        })?;
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| JevError::InputInvalid {
                detail: "question is not a choice question with two or more criteria".to_owned(),
            })?;
        let choices = item
            .get("choices")
            .and_then(Value::as_array)
            .ok_or_else(|| JevError::InputInvalid {
                detail: format!("question {id} is not a choice question with two or more criteria"),
            })?;
        let mut labels = BTreeSet::new();
        for choice in choices {
            let label = choice.as_str().ok_or_else(|| JevError::InputInvalid {
                detail: format!("question {id} is not a choice question with two or more criteria"),
            })?;
            labels.insert(label.to_owned());
        }
        if labels.len() < 2 {
            return Err(JevError::InputInvalid {
                detail: format!("question {id} is not a choice question with two or more criteria"),
            });
        }
        if criteria.insert(id.to_owned(), labels).is_some() {
            return Err(JevError::InputInvalid {
                detail: format!("question id {id} is duplicated"),
            });
        }
    }
    Ok(criteria)
}

fn splice_model(model: &str, payload: &[u8]) -> Result<Vec<u8>, JevError> {
    let model_json = serde_json::to_string(model).map_err(|error| JevError::ConfigInvalid {
        detail: format!("Jev model id is not valid JSON: {error}"),
    })?;
    let mut body = Vec::with_capacity(payload.len() + model_json.len() + 10);
    body.extend_from_slice(br#"{"model":"#);
    body.extend_from_slice(model_json.as_bytes());
    if payload == b"{}" {
        body.push(b'}');
    } else {
        body.push(b',');
        body.extend_from_slice(&payload[1..]);
    }
    Ok(body)
}

fn require_number(value: Option<&Value>) -> Result<(), JevError> {
    match value {
        Some(Value::Number(number)) if number.as_f64().is_some() => Ok(()),
        _ => Err(JevError::ResponseInvalid {
            detail: "Jev confidence or probability is not a number".to_owned(),
        }),
    }
}

#[cfg(test)]
mod jev_api {
    #[test]
    fn retry_delay_matches_the_readme_clamp() {
        use std::time::Duration;

        use crate::client::retry_delay;

        assert_eq!(retry_delay(None), Duration::from_secs(1));
        assert_eq!(retry_delay(Some("")), Duration::from_secs(1));
        assert_eq!(retry_delay(Some("2")), Duration::from_secs(2));
        assert_eq!(retry_delay(Some("2.5")), Duration::from_secs_f64(2.5));
        assert_eq!(retry_delay(Some("100")), Duration::from_secs(30));
        assert_eq!(retry_delay(Some("-4")), Duration::from_secs(0));
        assert_eq!(retry_delay(Some("nope")), Duration::from_secs(30));
        assert_eq!(retry_delay(Some("   ")), Duration::from_secs(30));
    }
}
