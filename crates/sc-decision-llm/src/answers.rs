//! The schema check: the model's JSON -> one Jev answer per question.
//!
//! A provider's strict mode is not trusted: every rule of the required schema
//! ([`crate::schema`]) is checked again here. Values pass through unrounded and unchecked for
//! range or sum (REQ-SAG-0007). Model-written names and values placed in an error message are
//! cut with [`bounded`] (RSH-002).

use serde_json::{Map, Value};

use sc_decision_schema::{
    Answer, BoxCoordinates, OrderedMap, Question, SystemOneRequest, SystemOneResponse, Usage,
};
use sc_llm_provider::LlmResponse;

use crate::DecisionLlmError;
use crate::error::bounded;
use crate::schema::score_keys;

/// The `answers` object of a parsed reply.
#[derive(Debug, Clone)]
pub(crate) struct LlmOutput {
    answers: Map<String, Value>,
}

impl LlmOutput {
    /// The `answers` object of the reply value, taken by value (not cloned).
    ///
    /// # Errors
    ///
    /// [`DecisionLlmError::AnswerIds`], every question missing, when there is no `answers`
    /// object.
    pub(crate) fn from_value(
        value: Value,
        question_ids: &[&str],
    ) -> Result<Self, DecisionLlmError> {
        match value {
            Value::Object(mut object) => match object.remove("answers") {
                Some(Value::Object(answers)) => Ok(Self { answers }),
                _ => Err(ids_differ(question_ids, &[])),
            },
            _ => Err(ids_differ(question_ids, &[])),
        }
    }
}

/// `AnswerIds`: `missing` are request ids (ours); `extra` are model-written, so at most
/// [`DecisionLlmError::MAX_LISTED_IDS`] are kept, each bounded, with the full count.
fn ids_differ(missing: &[&str], extra: &[&str]) -> DecisionLlmError {
    DecisionLlmError::AnswerIds {
        missing: missing.iter().map(|id| (*id).to_owned()).collect(),
        extra: extra
            .iter()
            .take(DecisionLlmError::MAX_LISTED_IDS)
            .map(|id| bounded(id))
            .collect(),
        extra_count: extra.len(),
    }
}

/// Checks `reply` against the required schema of `request` and returns its answers in
/// request order, with the provider-reported model and usage.
///
/// # Errors
///
/// - [`DecisionLlmError::Encode`] when a question type is unsupported (refused before any
///   answer is read, never a mismatch);
/// - [`DecisionLlmError::Invariant`] when `reply.json` is `None` (the backend always asks for
///   `Output::Json`, so sc-llm-provider always fills it) or a built map gets a key twice;
/// - [`DecisionLlmError::AnswerIds`] when the answer ids differ from the question ids;
/// - [`DecisionLlmError::AnswerInvalid`] when one answer lacks a field, has a wrong type, an
///   extra field, or a choice outside the criteria.
pub(crate) fn answers(
    request: &SystemOneRequest,
    reply: LlmResponse,
) -> Result<SystemOneResponse, DecisionLlmError> {
    let LlmResponse {
        model, json, usage, ..
    } = reply;
    let ids: Vec<&str> = request
        .questions
        .iter()
        .map(|(id, _)| id.as_str())
        .collect();
    for (id, question) in &request.questions {
        if !is_known(question) {
            return Err(DecisionLlmError::Encode {
                message: format!(
                    "question `{id}` has a type the LLM backend does not support (noul, choice, score, box)"
                ),
            });
        }
    }
    let json = json.ok_or_else(|| DecisionLlmError::Invariant {
        message: "the reply has no parsed JSON although Output::Json was asked for".to_owned(),
    })?;
    let output = LlmOutput::from_value(json, &ids)?;
    let missing: Vec<&str> = ids
        .iter()
        .copied()
        .filter(|id| !output.answers.contains_key(*id))
        .collect();
    let extra: Vec<&str> = output
        .answers
        .keys()
        .map(String::as_str)
        .filter(|key| !ids.contains(key))
        .collect();
    if !missing.is_empty() || !extra.is_empty() {
        return Err(ids_differ(&missing, &extra));
    }
    let mut typed = Vec::with_capacity(ids.len());
    for (id, question) in &request.questions {
        let item = output
            .answers
            .get(id)
            .and_then(Value::as_object)
            .ok_or_else(|| invalid(id, "the answer is not an object".to_owned()))?;
        typed.push((id.clone(), answer(id, question, item)?));
    }
    Ok(SystemOneResponse::new(
        model,
        ordered(typed)?,
        Usage {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            extra: Map::new(),
        },
        Map::new(),
    ))
}

fn is_known(question: &Question) -> bool {
    matches!(
        question,
        Question::Noul { .. }
            | Question::Choice { .. }
            | Question::Score { .. }
            | Question::Box { .. }
    )
}

/// `pairs` as an [`OrderedMap`].
///
/// Every key here is unique by construction: question ids and choice criteria come from
/// `OrderedMap`s (which refuse duplicates), score keys are `0..n`. A duplicate would be a bug
/// in this crate, so it is [`DecisionLlmError::Invariant`] (never resent), not a mismatch.
fn ordered<V>(pairs: Vec<(String, V)>) -> Result<OrderedMap<String, V>, DecisionLlmError> {
    OrderedMap::try_from_pairs(pairs).map_err(|error| DecisionLlmError::Invariant {
        message: format!("an answer map got a key twice: {error}"),
    })
}

fn invalid(question: &str, message: String) -> DecisionLlmError {
    DecisionLlmError::AnswerInvalid {
        question: question.to_owned(),
        message,
    }
}

/// The `N` fields of one answer, in `allowed` order: every one present, nothing else.
fn fields<'a, const N: usize>(
    id: &str,
    item: &'a Map<String, Value>,
    allowed: [&str; N],
) -> Result<[&'a Value; N], DecisionLlmError> {
    if let Some(extra) = item.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(invalid(
            id,
            format!("unexpected field `{}`", bounded(extra)),
        ));
    }
    let mut out = [&Value::Null; N];
    for (slot, name) in out.iter_mut().zip(allowed) {
        *slot = item
            .get(name)
            .ok_or_else(|| invalid(id, format!("missing field `{name}`")))?;
    }
    Ok(out)
}

fn number(id: &str, field: &str, value: &Value) -> Result<f64, DecisionLlmError> {
    match value {
        Value::Number(number) => number
            .as_f64()
            .ok_or_else(|| invalid(id, format!("`{field}` is not a finite number"))),
        _ => Err(invalid(id, format!("`{field}` is not a number"))),
    }
}

/// An object with a number under exactly each of `keys`, in `keys` order.
fn probabilities(
    id: &str,
    value: &Value,
    keys: &[String],
) -> Result<OrderedMap<String, f64>, DecisionLlmError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(id, "`probabilities` is not an object".to_owned()))?;
    if let Some(extra) = object.keys().find(|key| !keys.contains(key)) {
        return Err(invalid(
            id,
            format!(
                "`probabilities` has `{}`, which is not an option",
                bounded(extra)
            ),
        ));
    }
    let pairs = keys
        .iter()
        .map(|key| {
            let value = object
                .get(key)
                .ok_or_else(|| invalid(id, format!("`probabilities` has no `{key}`")))?;
            Ok((
                key.clone(),
                number(id, &format!("probabilities.{key}"), value)?,
            ))
        })
        .collect::<Result<Vec<_>, DecisionLlmError>>()?;
    ordered(pairs)
}

const EDGES: [&str; 4] = ["top", "bottom", "left", "right"];

fn answer(
    id: &str,
    question: &Question,
    item: &Map<String, Value>,
) -> Result<Answer, DecisionLlmError> {
    match question {
        Question::Noul { .. } => {
            let [noul] = fields(id, item, ["noul"])?;
            Ok(Answer::Noul {
                noul: number(id, "noul", noul)?,
            })
        }
        Question::Choice { criteria, .. } => {
            let keys: Vec<String> = criteria.iter().map(|(key, _)| key.clone()).collect();
            let [choice, confidence, probs] =
                fields(id, item, ["choice", "confidence", "probabilities"])?;
            let choice = choice
                .as_str()
                .ok_or_else(|| invalid(id, "`choice` is not a string".to_owned()))?;
            if !keys.iter().any(|key| key == choice) {
                return Err(invalid(
                    id,
                    format!("choice `{}` is not one of the criteria", bounded(choice)),
                ));
            }
            Ok(Answer::Choice {
                choice: choice.to_owned(),
                confidence: number(id, "confidence", confidence)?,
                probabilities: probabilities(id, probs, &keys)?,
            })
        }
        Question::Score { criteria, .. } => {
            let keys = score_keys(criteria.len());
            let [score, confidence, probs] =
                fields(id, item, ["score", "confidence", "probabilities"])?;
            Ok(Answer::Score {
                score: number(id, "score", score)?,
                confidence: number(id, "confidence", confidence)?,
                legend: ordered(keys.iter().cloned().zip(criteria.iter().cloned()).collect())?,
                probabilities: probabilities(id, probs, &keys)?,
            })
        }
        Question::Box { .. } => {
            let [edges, confidence] = fields(id, item, ["box", "confidence"])?;
            let edges = edges
                .as_object()
                .ok_or_else(|| invalid(id, "`box` is not an object".to_owned()))?;
            if let Some(extra) = edges.keys().find(|key| !EDGES.contains(&key.as_str())) {
                return Err(invalid(
                    id,
                    format!("`box` has unexpected `{}`", bounded(extra)),
                ));
            }
            let edge = |name: &str| {
                let value = edges
                    .get(name)
                    .ok_or_else(|| invalid(id, format!("`box` has no `{name}`")))?;
                number(id, &format!("box.{name}"), value)
            };
            Ok(Answer::Box {
                coordinates: BoxCoordinates {
                    top: edge("top")?,
                    bottom: edge("bottom")?,
                    left: edge("left")?,
                    right: edge("right")?,
                },
                confidence: match confidence {
                    Value::Null => None,
                    other => Some(number(id, "confidence", other)?),
                },
            })
        }
        // Unreachable: `answers` refuses an unknown type (is_known) before any answer.
        // Kept as the same non-mismatch refusal rather than a panic.
        _ => Err(DecisionLlmError::Encode {
            message: format!("question `{id}` has a type the LLM backend does not support"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use sc_agent_types::ErrorId;

    use super::*;

    /// QA2-001 (mutant N3b): a duplicate key in a built map is a bug of this crate,
    /// decision_llm.invariant, not an answer the runner would resend.
    #[test]
    fn duplicate_key_is_an_invariant_failure() {
        let error = ordered(vec![("q".to_owned(), 1.0), ("q".to_owned(), 2.0)]).unwrap_err();
        assert_eq!(error.id(), "decision_llm.invariant");
        assert!(matches!(error, DecisionLlmError::Invariant { .. }));
        let map = ordered(vec![("a".to_owned(), 1.0), ("b".to_owned(), 2.0)]).unwrap();
        assert_eq!(map.len(), 2);
    }
}
