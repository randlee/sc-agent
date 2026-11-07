//! Question-set and state documents merged into a payload.

use std::collections::{BTreeMap, BTreeSet};

use sc_agent_types::{ChoiceAnswer, ChoiceId, JevError, Question, QuestionId};
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
    let raw: RawQuestionSet = serde_json::from_str(text)
        .map_err(|error| JevError::input_invalid(format!("question set: {error}")))?;
    if raw.schema != QUESTION_SET_SCHEMA {
        return Err(JevError::input_invalid(format!(
            "question set schema is {}, expected {QUESTION_SET_SCHEMA}",
            raw.schema
        )));
    }
    if raw.version.is_some_and(|version| version == 0) {
        return Err(JevError::input_invalid(
            "question set version must be greater than zero",
        ));
    }
    // gate, notes, and roles are accepted and are not part of the judge payload.
    let _context = (raw.gate, raw.notes, raw.roles);
    if raw.questions.is_empty() {
        return Err(JevError::input_invalid("question set has no questions"));
    }
    let mut questions = Vec::with_capacity(raw.questions.len());
    for (id, question) in raw.questions {
        if question.kind != "choice" {
            return Err(JevError::input_invalid(format!(
                "question {id} type {} is not choice",
                question.kind
            )));
        }
        if question.criteria.is_empty()
            || question
                .criteria
                .values()
                .any(|value| value.trim().is_empty())
        {
            return Err(JevError::input_invalid(format!(
                "question {id} criteria are empty"
            )));
        }
        let choices = question
            .criteria
            .into_keys()
            .map(ChoiceId::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        questions.push(Question::new(
            QuestionId::try_from(id)?,
            question.instructions,
            choices,
        ));
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
        return Err(JevError::input_invalid("question set has no questions"));
    }
    let mut seen = BTreeSet::new();
    for question in questions {
        let id = question.id();
        if !seen.insert(id.as_str()) {
            return Err(JevError::input_invalid(format!(
                "question id {id} is repeated"
            )));
        }
        if question.prompt().trim().is_empty() {
            return Err(JevError::input_invalid(format!(
                "question {id} instructions are empty"
            )));
        }
        if question.choices().is_empty() {
            return Err(JevError::input_invalid(format!(
                "question {id} choices are empty"
            )));
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
            return Err(JevError::input_invalid(format!(
                "state repeats question {}",
                answer.question_id
            )));
        }
        let Some(question) = questions
            .iter()
            .find(|question| question.id() == &answer.question_id)
        else {
            return Err(JevError::input_invalid(format!(
                "state question {} is not in the question set",
                answer.question_id
            )));
        };
        if !question
            .choices()
            .iter()
            .any(|choice| choice == &answer.choice)
        {
            return Err(JevError::input_invalid(format!(
                "state question {} has choice {}, which is not in the question's choices",
                answer.question_id, answer.choice
            )));
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
    let raw: RawState = serde_json::from_str(text)
        .map_err(|error| JevError::input_invalid(format!("state: {error}")))?;
    if raw.schema != STATE_SCHEMA {
        return Err(JevError::input_invalid(format!(
            "state schema is {}, expected {STATE_SCHEMA}",
            raw.schema
        )));
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
        Question::new(
            QuestionId::try_from(id).expect("id"),
            prompt,
            choices
                .iter()
                .map(|choice| ChoiceId::try_from(*choice).expect("choice"))
                .collect(),
        )
    }

    fn answer(id: &str, choice: &str) -> ChoiceAnswer {
        ChoiceAnswer {
            question_id: QuestionId::try_from(id).expect("id"),
            choice: ChoiceId::try_from(choice).expect("choice"),
        }
    }

    #[test]
    fn question_set_rules() {
        assert!(validate_question_set(&[question("q", "p", &["a"])]).is_ok());
        for bad in [
            vec![],
            vec![question("q", " ", &["a"])],
            vec![question("q", "p", &[])],
            vec![question("q", "p", &["a"]), question("q", "p", &["a"])],
        ] {
            let error = validate_question_set(&bad).expect_err("invalid set");
            assert_eq!(error.code(), "INPUT_INVALID");
        }
        // Empty ids and choices are unrepresentable (`QuestionId`, `ChoiceId`).
        assert!(QuestionId::try_from("").is_err());
        assert!(ChoiceId::try_from("").is_err());
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
