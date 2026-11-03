//! In-process judge used when Python passes `scripted_answers`.
//!
//! This backend never opens a socket and never stores an API key. Production
//! runs omit it and let `sc-jev-check` construct the HTTP backends.

use std::sync::Arc;

use sc_agent_types::{BackendId, JevError, JudgeBackend, JudgeOutcome, JudgeRequest};
use sc_jev_check::registry::BackendRegistry;

/// Returns `answers` for every accepted request.
pub(crate) struct ScriptedJudge {
    id: BackendId,
    answers: serde_json::Value,
}

impl ScriptedJudge {
    fn new(id: BackendId, answers: serde_json::Value) -> Self {
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
            answers: self.answers.clone(),
            backend: self.id,
            probabilities: None,
            provider_meta: None,
        })
    }
}

/// Registers one scripted judge per `ids` entry.
///
/// # Errors
///
/// Returns [`JevError::registry_invalid`] when `ids` repeats a backend.
pub(crate) fn registry(
    ids: &[BackendId],
    answers: serde_json::Value,
) -> Result<BackendRegistry, JevError> {
    let mut registry = BackendRegistry::new();
    for id in ids {
        registry.register(Arc::new(ScriptedJudge::new(*id, answers.clone())))?;
    }
    Ok(registry)
}

fn is_llm(id: BackendId) -> bool {
    matches!(id, BackendId::AnthropicHaiku | BackendId::OpenAiLuna)
}
