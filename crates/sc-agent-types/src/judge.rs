//! Frozen judge contract (ADR-SAG-0019).
//!
//! The facade builds one [`PayloadBundle`] and sets [`JudgeRequest::llm`] only
//! for Haiku and Luna. Implementations hold transport and the resolved API key
//! at construction. This trait does not name a transport or accept secrets.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::JevError;
use crate::ids::{BackendId, Sha256Digest};
use crate::subjects::SubjectsProducer;

/// One question the judge answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Question {
    /// Caller-chosen question id.
    pub id: String,
    /// Instructions sent with the question.
    pub prompt: String,
    /// Choice labels, in display order.
    pub choices: Vec<String>,
}

/// A recorded choice for one question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChoiceAnswer {
    /// [`Question::id`] this answer belongs to.
    pub question_id: String,
    /// Selected choice label.
    pub choice: String,
}

/// Questions plus state, before canonical encoding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalJudgePayload {
    /// Questions the backends must judge.
    pub questions: Vec<Question>,
    /// Prior answers carried into this request.
    pub state: Vec<ChoiceAnswer>,
}

/// Canonical judge payload bytes and their SHA-256.
///
/// `sha256` is computed on [`Self::bytes`] only. LLM system-prompt text is not
/// part of the digest (ADR-SAG-0019).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadBundle {
    /// Sorted-key compact JSON, UTF-8.
    pub bytes: Vec<u8>,
    /// SHA-256 of `bytes`.
    pub sha256: Sha256Digest,
}

impl PayloadBundle {
    /// Wraps bytes that are already canonical and records their digest.
    #[must_use]
    pub fn from_canonical_bytes(bytes: Vec<u8>) -> Self {
        let sha256 = Sha256Digest::of(&bytes);
        Self { bytes, sha256 }
    }
}

/// Caller-supplied evaluator instructions for Haiku and Luna.
///
/// Jev-only runs leave [`JudgeRequest::llm`] empty. The text is not part of
/// `prompt_sha256`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmWireContext {
    /// System prompt resolved by the facade.
    pub system_prompt: String,
}

/// One judge call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgeRequest {
    /// Canonical payload shared by every backend in the run.
    pub payload: PayloadBundle,
    /// System prompt for LLM backends. `None` for Jev-only runs.
    pub llm: Option<LlmWireContext>,
}

/// Normalized TypeSafe Jev answer both backend families emit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JudgeOutcome {
    /// Answer object keyed by question id.
    pub answers: Value,
    /// Backend that produced `answers`.
    pub backend: BackendId,
    /// Jev probability fields when present. `None` for LLM backends.
    pub probabilities: Option<Value>,
    /// Provider metadata for debugging. Not part of equal-input.
    pub provider_meta: Option<Value>,
}

/// Equal-input attestation for one check run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EqualInputRecord {
    /// SHA-256 of the canonical payload bytes.
    pub prompt_sha256: Sha256Digest,
    /// Whether any backend was withheld from the comparison.
    pub leak: bool,
    /// Backend command or id strings recorded for the run.
    pub commands: Vec<String>,
    /// Whether every participating backend saw the same canonical bytes.
    pub equal: bool,
    /// Subjects `producer` block when the check used subjects or command inputs.
    pub producer: Option<SubjectsProducer>,
}

/// Sends one canonical payload to a judge.
///
/// HTTP clients and API keys are construction state on the implementor.
/// Later crates (`sc-jev`, `sc-jev-backends`, and a future local judge)
/// implement this trait. It is intentionally open.
pub trait JudgeBackend: Send + Sync {
    /// Returns the backend id.
    fn id(&self) -> BackendId;

    /// Refuses the request before any network send.
    ///
    /// # Errors
    ///
    /// Returns [`JevError`] when the payload exceeds this backend's limit or
    /// the request is otherwise not sendable.
    fn validate_for_send(&self, req: &JudgeRequest) -> Result<(), JevError>;

    /// Judges `req` and returns the normalized outcome.
    ///
    /// # Errors
    ///
    /// Returns [`JevError`] when validation or the judge call fails.
    fn judge(&self, req: &JudgeRequest) -> Result<JudgeOutcome, JevError>;
}
