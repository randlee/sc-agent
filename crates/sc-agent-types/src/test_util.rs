//! Test doubles for the frozen judge contract.
//!
//! Enable the `test-util` feature to use these types from another crate.
//! Crate unit tests compile this module without the feature.

use crate::error::JevError;
use crate::ids::BackendId;
use crate::judge::{JudgeBackend, JudgeOutcome, JudgeRequest};

/// Judge backend that returns a fixed outcome or a fixed refusal.
#[derive(Debug, Clone)]
pub struct MockJudgeBackend {
    id: BackendId,
    outcome: JudgeOutcome,
    refusal: Option<JevError>,
}

impl MockJudgeBackend {
    /// Returns `outcome` for every accepted request.
    #[must_use]
    pub fn new(id: BackendId, outcome: JudgeOutcome) -> Self {
        Self {
            id,
            outcome,
            refusal: None,
        }
    }

    /// Fails `validate_for_send` and `judge` with `error`.
    #[must_use]
    pub fn refusing(id: BackendId, error: JevError) -> Self {
        Self {
            id,
            outcome: JudgeOutcome {
                answers: serde_json::json!({}),
                backend: id,
                probabilities: None,
                provider_meta: None,
            },
            refusal: Some(error),
        }
    }
}

impl JudgeBackend for MockJudgeBackend {
    fn id(&self) -> BackendId {
        self.id
    }

    fn validate_for_send(&self, _req: &JudgeRequest) -> Result<(), JevError> {
        match &self.refusal {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    fn judge(&self, req: &JudgeRequest) -> Result<JudgeOutcome, JevError> {
        self.validate_for_send(req)?;
        let mut outcome = self.outcome.clone();
        outcome.backend = self.id;
        Ok(outcome)
    }
}

/// Field names every `JudgeOutcome` carries.
///
/// `sc-jev` and `sc-jev-backends` must fill this same set. Those crates are
/// stubs in sprint a-1, so the contract test below covers [`MockJudgeBackend`]
/// only. Later sprints call this helper on their backends.
#[must_use]
pub fn judge_outcome_field_names() -> &'static [&'static str] {
    &["answers", "backend", "probabilities", "provider_meta"]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::Sha256Digest;
    use crate::judge::{JudgeRequest, LlmWireContext, PayloadBundle};
    use serde_json::Value;

    fn request() -> JudgeRequest {
        JudgeRequest {
            payload: PayloadBundle::from_canonical_bytes(b"{}".to_vec()),
            llm: Some(LlmWireContext {
                system_prompt: "evaluate".into(),
            }),
        }
    }

    fn outcome() -> JudgeOutcome {
        JudgeOutcome {
            answers: serde_json::json!({"relevant": {"choice": "yes"}}),
            backend: BackendId::Jev,
            probabilities: Some(serde_json::json!({"yes": 0.9})),
            provider_meta: Some(serde_json::json!({"model": "mock"})),
        }
    }

    #[test]
    fn mock_fills_every_judge_outcome_field() {
        let backend = MockJudgeBackend::new(BackendId::AnthropicHaiku, outcome());
        assert_eq!(backend.id(), BackendId::AnthropicHaiku);
        let judged = backend.judge(&request()).expect("judge");
        let value = serde_json::to_value(&judged).expect("serialize");
        let object = value.as_object().expect("object");
        let names: Vec<&str> = object.keys().map(String::as_str).collect();
        assert_eq!(names, judge_outcome_field_names());
        assert_eq!(judged.backend, BackendId::AnthropicHaiku);
        assert!(judged.probabilities.is_some());
        assert!(judged.provider_meta.is_some());
        assert!(matches!(judged.answers, Value::Object(_)));
        let req = request();
        assert_eq!(req.payload.sha256, Sha256Digest::of(&req.payload.bytes));
    }

    #[test]
    fn mock_refusal_does_not_return_an_outcome() {
        let backend = MockJudgeBackend::refusing(
            BackendId::Jev,
            JevError::InputTooLarge {
                limit_bytes: 4,
                actual_bytes: 8,
            },
        );
        let error = backend.judge(&request()).expect_err("refused");
        assert_eq!(error.code(), "INPUT_TOO_LARGE");
        assert!(!error.fix_hint().is_empty());
    }
}
