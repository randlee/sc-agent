//! Question-set and state documents merged into a payload.

use std::collections::BTreeMap;

use sc_agent_types::{ChoiceAnswer, JevError, Question};
use serde::Deserialize;

/// Schema id required on question-set files.
pub const QUESTION_SET_SCHEMA: &str = "plan-anchoring/questions/1";

/// Schema id required on state files.
pub const STATE_SCHEMA: &str = "plan-anchoring/state/1";

/// Parses a `plan-anchoring/questions/1` document into judge questions.
///
/// Choices are the criteria keys in lexicographic order. `prompt` is the
/// question `instructions` text, before input substitution.
///
/// # Errors
///
/// Returns [`JevError::InputInvalid`] when the schema, question type, or
/// criteria do not match the Phase A question set.
pub fn parse_question_set(text: &str) -> Result<Vec<Question>, JevError> {
    let raw: RawQuestionSet =
        serde_json::from_str(text).map_err(|error| JevError::InputInvalid {
            detail: format!("question set: {error}"),
        })?;
    if raw.schema != QUESTION_SET_SCHEMA {
        return Err(JevError::InputInvalid {
            detail: format!(
                "question set schema is {}, expected {QUESTION_SET_SCHEMA}",
                raw.schema
            ),
        });
    }
    if raw.version.is_some_and(|version| version == 0) {
        return Err(JevError::InputInvalid {
            detail: "question set version must be greater than zero".into(),
        });
    }
    // gate, notes, and roles are accepted and are not part of the judge payload.
    let _context = (raw.gate, raw.notes, raw.roles);
    if raw.questions.is_empty() {
        return Err(JevError::InputInvalid {
            detail: "question set has no questions".into(),
        });
    }
    let mut questions = Vec::with_capacity(raw.questions.len());
    for (id, question) in raw.questions {
        if id.is_empty() {
            return Err(JevError::InputInvalid {
                detail: "question id is empty".into(),
            });
        }
        if question.kind != "choice" {
            return Err(JevError::InputInvalid {
                detail: format!("question {id} type {} is not choice", question.kind),
            });
        }
        if question.instructions.trim().is_empty() {
            return Err(JevError::InputInvalid {
                detail: format!("question {id} instructions are empty"),
            });
        }
        if question.criteria.is_empty()
            || question
                .criteria
                .values()
                .any(|value| value.trim().is_empty())
        {
            return Err(JevError::InputInvalid {
                detail: format!("question {id} criteria are empty"),
            });
        }
        questions.push(Question {
            id,
            prompt: question.instructions,
            choices: question.criteria.into_keys().collect(),
        });
    }
    Ok(questions)
}

/// Parses a `plan-anchoring/state/1` document and checks it against `questions`.
///
/// # Errors
///
/// Returns [`JevError::InputInvalid`] when the schema is wrong, a question id
/// is unknown, or a choice is not one of that question's choices.
pub fn parse_state(text: &str, questions: &[Question]) -> Result<Vec<ChoiceAnswer>, JevError> {
    let raw: RawState = serde_json::from_str(text).map_err(|error| JevError::InputInvalid {
        detail: format!("state: {error}"),
    })?;
    if raw.schema != STATE_SCHEMA {
        return Err(JevError::InputInvalid {
            detail: format!("state schema is {}, expected {STATE_SCHEMA}", raw.schema),
        });
    }
    let mut seen = BTreeMap::<&str, ()>::new();
    for answer in &raw.answers {
        if seen.insert(answer.question_id.as_str(), ()).is_some() {
            return Err(JevError::InputInvalid {
                detail: format!("state repeats question {}", answer.question_id),
            });
        }
        let Some(question) = questions
            .iter()
            .find(|question| question.id == answer.question_id)
        else {
            return Err(JevError::InputInvalid {
                detail: format!(
                    "state question {} is not in the question set",
                    answer.question_id
                ),
            });
        };
        if !question
            .choices
            .iter()
            .any(|choice| choice == &answer.choice)
        {
            return Err(JevError::InputInvalid {
                detail: format!(
                    "state choice {} is not valid for {}",
                    answer.choice, answer.question_id
                ),
            });
        }
    }
    Ok(raw.answers)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawQuestionSet {
    schema: String,
    #[serde(default)]
    gate: Option<String>,
    #[serde(default)]
    version: Option<u64>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    roles: BTreeMap<String, String>,
    questions: BTreeMap<String, RawQuestion>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawQuestion {
    #[serde(rename = "type")]
    kind: String,
    instructions: String,
    criteria: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawState {
    schema: String,
    answers: Vec<ChoiceAnswer>,
}
