//! In-process judges for tests (ADR-SAG-0022).
//!
//! [`register_scripted_backends`] builds a [`BackendRegistry`] whose judges
//! return a fixed answer object and never open a socket. Production builds
//! leave the `test-util` feature off, so [`crate::run::run_check`] stays the
//! caller API (ADR-SAG-0009).

use std::sync::Arc;

use sc_agent_types::{BackendId, JevError, JudgeBackend, JudgeOutcome, JudgeRequest};

use super::BackendRegistry;

/// Returns `answers` for every accepted request.
struct ScriptedJudge {
    id: BackendId,
    answers: Arc<serde_json::Value>,
}

impl ScriptedJudge {
    fn new(id: BackendId, answers: Arc<serde_json::Value>) -> Self {
        Self { id, answers }
    }
}

impl std::fmt::Debug for ScriptedJudge {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ScriptedJudge")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl JudgeBackend for ScriptedJudge {
    fn id(&self) -> BackendId {
        self.id
    }

    fn validate_for_send(&self, request: &JudgeRequest) -> Result<(), JevError> {
        if is_llm(self.id) && request.llm.is_none() {
            return Err(JevError::InputInvalid {
                detail: "LLM system prompt is required".into(),
            });
        }
        Ok(())
    }

    fn judge(&self, request: &JudgeRequest) -> Result<JudgeOutcome, JevError> {
        self.validate_for_send(request)?;
        Ok(JudgeOutcome {
            answers: self.answers.as_ref().clone(),
            backend: self.id,
            probabilities: None,
            provider_meta: None,
        })
    }
}

/// Registers one scripted judge per `ids` entry.
///
/// Each judge returns `answers` and does not open a socket or store an API
/// key. Callers pass the registry to [`crate::run::RunOptions::registry`].
/// This does not change [`crate::run::run_check`]'s parameters (ADR-SAG-0009).
///
/// # Errors
///
/// Returns [`JevError::registry_invalid`] when `ids` repeats a backend.
pub fn register_scripted_backends(
    ids: &[BackendId],
    answers: serde_json::Value,
) -> Result<BackendRegistry, JevError> {
    let mut registry = BackendRegistry::new();
    for judge in shared_judges(ids, answers) {
        registry.register(Arc::new(judge))?;
    }
    Ok(registry)
}

fn shared_judges(ids: &[BackendId], answers: serde_json::Value) -> Vec<ScriptedJudge> {
    let shared = Arc::new(answers);
    ids.iter()
        .copied()
        .map(|id| ScriptedJudge::new(id, Arc::clone(&shared)))
        .collect()
}

fn is_llm(id: BackendId) -> bool {
    matches!(id, BackendId::AnthropicHaiku | BackendId::OpenAiLuna)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripted_answers_share_one_allocation() {
        let judges = shared_judges(
            &[BackendId::Jev, BackendId::OpenAiLuna],
            serde_json::json!({"relevant": "yes"}),
        );
        assert_eq!(judges.len(), 2);
        assert!(Arc::ptr_eq(&judges[0].answers, &judges[1].answers));
    }
}
