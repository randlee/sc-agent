//! Question-set and state documents merged into a payload.

use std::collections::{BTreeMap, BTreeSet};

use sc_agent_types::{ChoiceAnswer, JevError, Question};
use serde::Deserialize;

/// Schema id required on question-set files.
pub const QUESTION_SET_SCHEMA: &str = "plan-anchoring/questions/1";

/// Schema id required on state files.
pub(crate) const STATE_SCHEMA: &str = "plan-anchoring/state/1";

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
        if question.kind != "choice" {
            return Err(JevError::InputInvalid {
                detail: format!("question {id} type {} is not choice", question.kind),
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
    validate_question_set(&questions)?;
    Ok(questions)
}

/// Checks the judge-question rules shared by `check.toml` and caller JSON.
///
/// The set is non-empty, every id is non-empty and unique, every prompt
/// (`instructions`) has text, and every question has at least one non-empty
/// choice.
///
/// # Errors
///
/// Returns [`JevError::InputInvalid`] naming the first rule that fails.
pub(crate) fn validate_question_set(questions: &[Question]) -> Result<(), JevError> {
    if questions.is_empty() {
        return Err(JevError::InputInvalid {
            detail: "question set has no questions".into(),
        });
    }
    let mut seen = BTreeSet::new();
    for question in questions {
        if question.id.is_empty() {
            return Err(JevError::InputInvalid {
                detail: "question id is empty".into(),
            });
        }
        if !seen.insert(question.id.as_str()) {
            return Err(JevError::InputInvalid {
                detail: format!("question id {} is repeated", question.id),
            });
        }
        if question.prompt.trim().is_empty() {
            return Err(JevError::InputInvalid {
                detail: format!("question {} instructions are empty", question.id),
            });
        }
        if question.choices.is_empty() || question.choices.iter().any(String::is_empty) {
            return Err(JevError::InputInvalid {
                detail: format!("question {} choices are empty", question.id),
            });
        }
    }
    Ok(())
}

/// Checks state answers against `questions`.
///
/// Every answer names a question in the set, picks one of that question's
/// choices, and no question is answered twice.
///
/// # Errors
///
/// Returns [`JevError::InputInvalid`] naming the first answer that fails.
pub(crate) fn validate_state(
    answers: &[ChoiceAnswer],
    questions: &[Question],
) -> Result<(), JevError> {
    let mut seen = BTreeSet::new();
    for answer in answers {
        if !seen.insert(answer.question_id.as_str()) {
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
                    "state question {} has choice {}, which is not in the question's choices",
                    answer.question_id, answer.choice
                ),
            });
        }
    }
    Ok(())
}

/// Parses a `plan-anchoring/state/1` document and checks it against `questions`.
///
/// # Errors
///
/// Returns [`JevError::InputInvalid`] when the schema is wrong, a question id
/// is unknown, or a choice is not one of that question's choices.
pub(crate) fn parse_state(
    text: &str,
    questions: &[Question],
) -> Result<Vec<ChoiceAnswer>, JevError> {
    let raw: RawState = serde_json::from_str(text).map_err(|error| JevError::InputInvalid {
        detail: format!("state: {error}"),
    })?;
    if raw.schema != STATE_SCHEMA {
        return Err(JevError::InputInvalid {
            detail: format!("state schema is {}, expected {STATE_SCHEMA}", raw.schema),
        });
    }
    validate_state(&raw.answers, questions)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn question(id: &str, prompt: &str, choices: &[&str]) -> Question {
        Question {
            id: id.to_owned(),
            prompt: prompt.to_owned(),
            choices: choices.iter().map(|choice| (*choice).to_owned()).collect(),
        }
    }

    fn answer(id: &str, choice: &str) -> ChoiceAnswer {
        ChoiceAnswer {
            question_id: id.to_owned(),
            choice: choice.to_owned(),
        }
    }

    #[test]
    fn question_set_rules() {
        assert!(validate_question_set(&[question("q", "p", &["a"])]).is_ok());
        for bad in [
            vec![],
            vec![question("", "p", &["a"])],
            vec![question("q", " ", &["a"])],
            vec![question("q", "p", &[])],
            vec![question("q", "p", &[""])],
            vec![question("q", "p", &["a"]), question("q", "p", &["a"])],
        ] {
            let error = validate_question_set(&bad).expect_err("invalid set");
            assert_eq!(error.code(), "INPUT_INVALID");
        }
    }

    #[test]
    fn state_rules() {
        let questions = [question("q", "p", &["no", "yes"])];
        assert!(validate_state(&[answer("q", "yes")], &questions).is_ok());
        for bad in [
            vec![answer("other", "yes")],
            vec![answer("q", "maybe")],
            vec![answer("q", "yes"), answer("q", "no")],
        ] {
            let error = validate_state(&bad, &questions).expect_err("invalid state");
            assert_eq!(error.code(), "INPUT_INVALID");
        }
    }
}
