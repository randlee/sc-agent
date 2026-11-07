//! In-process judges for tests (ADR-SAG-0022, r-1 amendment).
//!
//! [`register_scripted_backends`] builds a [`BackendRegistry`] of
//! [`MockJudgeBackend`]s, the workspace's one judge double. Production builds
//! leave the `test-util` feature off; the scripted registry reaches a run
//! through [`crate::run::RunConfig::registry`] (ADR-SAG-0022 r-2 amendment).

use std::collections::BTreeMap;
use std::sync::Arc;

use sc_agent_types::test_util::MockJudgeBackend;
use sc_agent_types::{BackendId, JevError, QuestionAnswer, QuestionId};
use serde_json::Value;

use super::BackendRegistry;

/// Registers one scripted judge per `ids` entry.
///
/// `answers` is a JSON object keyed by question id. Each value is either an
/// answer object (`{"choice": "yes"}`, optionally with `confidence` and
/// `probabilities`) or the string shorthand `"yes"`, which is stored as
/// `{"choice": "yes"}`. Every caller (CLI tests, the Python binding's
/// `_testing` module) therefore accepts the same shape.
///
/// Each judge is a [`MockJudgeBackend`]: it answers at `judge` time against
/// the request it receives, does not open a socket, and stores no API key.
/// Callers pass the registry, in an `Arc`, to
/// [`crate::run::RunConfig::registry`]. No run API changes (ADR-SAG-0009).
///
/// # Errors
///
/// Returns `INPUT_INVALID` when `answers` is not an object or a value is
/// neither a string nor a valid answer object, and `CONFIG_INVALID`
/// ([`JevError::RegistryInvalid`]) when `ids` repeats a backend.
///
/// # Examples
///
/// Needs the `test-util` feature (a doctest links the library without
/// `cfg(test)`), so plain `cargo test -p sc-jev-check` does not build it.
///
#[cfg_attr(feature = "test-util", doc = "```")]
#[cfg_attr(not(feature = "test-util"), doc = "```ignore")]
/// use sc_agent_types::BackendId;
/// use sc_jev_check::registry::register_scripted_backends;
///
/// let registry =
///     register_scripted_backends(&[BackendId::Jev], serde_json::json!({"q": "yes"})).unwrap();
/// assert!(registry.get(BackendId::Jev).is_ok());
/// assert_eq!(
///     register_scripted_backends(&[BackendId::Jev], serde_json::json!(["yes"]))
///         .unwrap_err()
///         .code(),
///     "INPUT_INVALID"
/// );
/// ```
pub fn register_scripted_backends(
    ids: &[BackendId],
    answers: Value,
) -> Result<BackendRegistry, JevError> {
    let answers = normalize_answers(answers)?;
    let mut registry = BackendRegistry::new();
    for id in ids.iter().copied() {
        registry.register(Arc::new(MockJudgeBackend::new(id, answers.clone())))?;
    }
    Ok(registry)
}

/// Expands the `"choice"` string shorthand and types every answer.
fn normalize_answers(answers: Value) -> Result<BTreeMap<QuestionId, QuestionAnswer>, JevError> {
    let Value::Object(map) = answers else {
        return Err(JevError::input_invalid(
            "scripted answers must be a JSON object of question ids",
        ));
    };
    let mut normalized = BTreeMap::new();
    for (id, answer) in map {
        let wrapped = match answer {
            Value::String(choice) => serde_json::json!({ "choice": choice }),
            object @ Value::Object(_) => object,
            _ => {
                return Err(JevError::input_invalid(format!(
                    "scripted answer for {id} must be a string or object"
                )));
            }
        };
        let answer: QuestionAnswer = serde_json::from_value(wrapped).map_err(|error| {
            JevError::input_invalid(format!("scripted answer for {id} is invalid: {error}"))
        })?;
        normalized.insert(QuestionId::try_from(id)?, answer);
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_shorthand_and_object_answers_normalise_alike() {
        let shorthand = normalize_answers(serde_json::json!({"q": "yes"})).expect("shorthand");
        let object = normalize_answers(serde_json::json!({"q": {"choice": "yes"}})).expect("obj");
        assert_eq!(shorthand, object);
        assert_eq!(shorthand["q"].choice().as_str(), "yes");
        for bad in [
            serde_json::json!("yes"),
            serde_json::json!({"q": 1}),
            serde_json::json!({"q": {"choice": "yes", "confidence": 2.0}}),
        ] {
            let error = normalize_answers(bad).expect_err("bad shape");
            assert_eq!(error.code(), "INPUT_INVALID");
        }
    }
}
