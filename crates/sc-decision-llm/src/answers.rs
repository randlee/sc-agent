//! The schema check: the model's JSON -> one Jev answer per question.
//!
//! A provider's strict mode is not trusted: every rule of the required schema
//! ([`crate::schema`]) is checked again here. Values pass through unrounded and unchecked for
//! range or sum (REQ-SAG-0007).

use serde_json::{Map, Value};

use sc_decision_schema::{
    Answer, BoxCoordinates, OrderedMap, Question, SystemOneRequest, SystemOneResponse, Usage,
};
use sc_llm_provider::LlmResponse;

use crate::DecisionLlmError;
use crate::schema::score_keys;

/// What a model's reply parses to before each answer is checked.
#[derive(Debug, Clone)]
pub(crate) struct LlmOutput {
    answers: Map<String, Value>,
}

impl LlmOutput {
    /// Parses the model's reply text.
    ///
    /// # Errors
    ///
    /// [`DecisionLlmError::AnswerInvalid`] (no question) when the text is not JSON;
    /// [`DecisionLlmError::AnswerIds`] when it has no `answers` object.
    pub(crate) fn parse(text: &str, question_ids: &[&str]) -> Result<Self, DecisionLlmError> {
        let value: Value =
            serde_json::from_str(text).map_err(|error| DecisionLlmError::AnswerInvalid {
                question: String::new(),
                message: format!("the reply is not JSON: {error}"),
            })?;
        Self::from_value(value, question_ids)
    }

    /// The `answers` object of an already parsed reply.
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

fn ids_differ(missing: &[&str], extra: &[&str]) -> DecisionLlmError {
    DecisionLlmError::AnswerIds {
        missing: missing.iter().map(|id| (*id).to_owned()).collect(),
        extra: extra.iter().map(|id| (*id).to_owned()).collect(),
    }
}

/// Checks `reply` against the required schema of `request` and returns its answers in
/// request order, with the provider-reported model and usage.
///
/// # Errors
///
/// [`DecisionLlmError::AnswerIds`] when the answer ids differ from the question ids;
/// [`DecisionLlmError::AnswerInvalid`] when the reply is not JSON or one answer lacks a field,
/// has a wrong type, an extra field, or a choice outside the criteria.
pub(crate) fn answers(
    request: &SystemOneRequest,
    reply: &LlmResponse,
) -> Result<SystemOneResponse, DecisionLlmError> {
    let ids: Vec<&str> = request
        .questions
        .iter()
        .map(|(id, _)| id.as_str())
        .collect();
    let output = match &reply.json {
        Some(value) => LlmOutput::from_value(value.clone(), &ids)?,
        None => LlmOutput::parse(&reply.text, &ids)?,
    };
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
    for (id, question) in request.questions.iter() {
        let item = output
            .answers
            .get(id)
            .and_then(Value::as_object)
            .ok_or_else(|| invalid(id, "the answer is not an object"))?;
        typed.push((id.clone(), answer(id, question, item)?));
    }
    Ok(SystemOneResponse {
        model: reply.model.clone(),
        answers: ordered("", typed)?,
        usage: Usage {
            input_tokens: reply.usage.input_tokens,
            output_tokens: reply.usage.output_tokens,
            extra: Map::new(),
        },
        extra: Map::new(),
    })
}

/// `pairs` as an [`OrderedMap`]; keys come from the request or the criteria, so a duplicate
/// cannot occur, and one is reported as an invalid answer rather than a panic.
fn ordered<V>(
    question: &str,
    pairs: Vec<(String, V)>,
) -> Result<OrderedMap<String, V>, DecisionLlmError> {
    OrderedMap::try_from_pairs(pairs).map_err(|error| invalid(question, &error.to_string()))
}

fn invalid(question: &str, message: &str) -> DecisionLlmError {
    DecisionLlmError::AnswerInvalid {
        question: question.to_owned(),
        message: message.to_owned(),
    }
}

/// The fields of one answer: every `allowed` field present, nothing else.
fn fields<'a>(
    id: &str,
    item: &'a Map<String, Value>,
    allowed: &[&str],
) -> Result<Vec<&'a Value>, DecisionLlmError> {
    if let Some(extra) = item.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(invalid(id, &format!("unexpected field `{extra}`")));
    }
    allowed
        .iter()
        .map(|name| {
            item.get(*name)
                .ok_or_else(|| invalid(id, &format!("missing field `{name}`")))
        })
        .collect()
}

fn number(id: &str, field: &str, value: &Value) -> Result<f64, DecisionLlmError> {
    match value {
        Value::Number(number) => number
            .as_f64()
            .ok_or_else(|| invalid(id, &format!("`{field}` is not a finite number"))),
        _ => Err(invalid(id, &format!("`{field}` is not a number"))),
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
        .ok_or_else(|| invalid(id, "`probabilities` is not an object"))?;
    if let Some(extra) = object.keys().find(|key| !keys.contains(key)) {
        return Err(invalid(
            id,
            &format!("`probabilities` has `{extra}`, which is not an option"),
        ));
    }
    let pairs = keys
        .iter()
        .map(|key| {
            let value = object
                .get(key)
                .ok_or_else(|| invalid(id, &format!("`probabilities` has no `{key}`")))?;
            Ok((
                key.clone(),
                number(id, &format!("probabilities.{key}"), value)?,
            ))
        })
        .collect::<Result<Vec<_>, DecisionLlmError>>()?;
    ordered(id, pairs)
}

fn answer(
    id: &str,
    question: &Question,
    item: &Map<String, Value>,
) -> Result<Answer, DecisionLlmError> {
    match question {
        Question::Noul { .. } => match fields(id, item, &["noul"])?.as_slice() {
            [noul] => Ok(Answer::Noul {
                noul: number(id, "noul", noul)?,
            }),
            _ => Err(invalid(id, "unexpected field count")),
        },
        Question::Choice { criteria, .. } => {
            let keys: Vec<String> = criteria.iter().map(|(key, _)| key.clone()).collect();
            match fields(id, item, &["choice", "confidence", "probabilities"])?.as_slice() {
                [choice, confidence, probs] => {
                    let choice = choice
                        .as_str()
                        .ok_or_else(|| invalid(id, "`choice` is not a string"))?;
                    if !keys.iter().any(|key| key == choice) {
                        return Err(invalid(
                            id,
                            &format!("choice `{choice}` is not one of the criteria"),
                        ));
                    }
                    Ok(Answer::Choice {
                        choice: choice.to_owned(),
                        confidence: number(id, "confidence", confidence)?,
                        probabilities: probabilities(id, probs, &keys)?,
                    })
                }
                _ => Err(invalid(id, "unexpected field count")),
            }
        }
        Question::Score { criteria, .. } => {
            let keys = score_keys(criteria.len());
            match fields(id, item, &["score", "confidence", "probabilities"])?.as_slice() {
                [score, confidence, probs] => Ok(Answer::Score {
                    score: number(id, "score", score)?,
                    confidence: number(id, "confidence", confidence)?,
                    legend: ordered(
                        id,
                        keys.iter().cloned().zip(criteria.iter().cloned()).collect(),
                    )?,
                    probabilities: probabilities(id, probs, &keys)?,
                }),
                _ => Err(invalid(id, "unexpected field count")),
            }
        }
        Question::Box { .. } => match fields(id, item, &["box", "confidence"])?.as_slice() {
            [edges, confidence] => {
                let edges = edges
                    .as_object()
                    .ok_or_else(|| invalid(id, "`box` is not an object"))?;
                let edge = |name: &str| {
                    let value = edges
                        .get(name)
                        .ok_or_else(|| invalid(id, &format!("`box` has no `{name}`")))?;
                    number(id, &format!("box.{name}"), value)
                };
                if let Some(extra) = edges
                    .keys()
                    .find(|key| !["top", "bottom", "left", "right"].contains(&key.as_str()))
                {
                    return Err(invalid(id, &format!("`box` has unexpected `{extra}`")));
                }
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
            _ => Err(invalid(id, "unexpected field count")),
        },
    }
}
