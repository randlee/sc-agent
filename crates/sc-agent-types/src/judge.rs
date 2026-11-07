//! Judge contract (ADR-SAG-0019, ADR-SAG-0027).
//!
//! The facade builds one [`PayloadBundle`] and sets [`JudgeRequest::llm`] only
//! for backends whose descriptor requires LLM context. Implementations hold
//! transport and the resolved API key at construction. This trait does not
//! name a transport or accept secrets.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cancel::CancelToken;
use crate::error::JevError;
use crate::ids::{
    BackendDescriptor, BackendFamily, BackendId, BackendKeyId, ChoiceId, QuestionId, Sha256Digest,
};
use crate::payload::{ChoiceAnswer, PayloadBundle};
use crate::schema::subjects::SubjectsProducer;

/// Caller-supplied evaluator instructions for LLM backends.
///
/// Jev-only runs leave [`JudgeRequest::llm`] empty. The text is not part of
/// `prompt_sha256`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmWireContext {
    /// System prompt resolved by the facade.
    pub system_prompt: String,
}

impl LlmWireContext {
    /// Wraps a resolved system prompt.
    #[must_use]
    pub fn new(system_prompt: impl Into<String>) -> Self {
        Self {
            system_prompt: system_prompt.into(),
        }
    }
}

/// One judge call.
///
/// The payload is shared (`Arc`), so a run builds one bundle for every
/// backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgeRequest {
    payload: Arc<PayloadBundle>,
    llm: Option<LlmWireContext>,
    cancel: CancelToken,
}

impl JudgeRequest {
    /// A request for `payload` with optional LLM context and a fresh
    /// (never-cancelled) [`CancelToken`].
    #[must_use]
    pub fn new(payload: Arc<PayloadBundle>, llm: Option<LlmWireContext>) -> Self {
        Self {
            payload,
            llm,
            cancel: CancelToken::default(),
        }
    }

    /// Uses `cancel` instead of the default token.
    #[must_use]
    pub fn with_cancel(mut self, cancel: CancelToken) -> Self {
        self.cancel = cancel;
        self
    }

    /// The canonical payload shared by every backend in the run.
    #[must_use]
    pub fn payload(&self) -> &PayloadBundle {
        &self.payload
    }

    /// The shared handle to the payload.
    #[must_use]
    pub fn payload_arc(&self) -> &Arc<PayloadBundle> {
        &self.payload
    }

    /// System prompt for LLM backends. `None` for Jev-only runs.
    #[must_use]
    pub fn llm(&self) -> Option<&LlmWireContext> {
        self.llm.as_ref()
    }

    /// The run's cancellation token.
    #[must_use]
    pub fn cancel(&self) -> &CancelToken {
        &self.cancel
    }
}

/// A backend's answer to one question.
///
/// `confidence` and every probability are finite and in `[0, 1]`;
/// [`Self::new`] and `Deserialize` enforce it, so an invalid backend
/// response never becomes a `QuestionAnswer`. Serialized keys are `choice`,
/// `confidence`, `probabilities`, each optional key omitted when `None`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawQuestionAnswer")]
pub struct QuestionAnswer {
    choice: ChoiceId,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    probabilities: Option<BTreeMap<ChoiceId, f64>>,
}

#[derive(Deserialize)]
struct RawQuestionAnswer {
    choice: ChoiceId,
    #[serde(default)]
    confidence: Option<f64>,
    #[serde(default)]
    probabilities: Option<BTreeMap<ChoiceId, f64>>,
}

impl TryFrom<RawQuestionAnswer> for QuestionAnswer {
    type Error = JevError;

    fn try_from(raw: RawQuestionAnswer) -> Result<Self, Self::Error> {
        Self::new(raw.choice, raw.confidence, raw.probabilities)
    }
}

impl QuestionAnswer {
    /// Builds an answer.
    ///
    /// # Errors
    ///
    /// `RESPONSE_INVALID` when `confidence` or a probability is NaN,
    /// infinite, negative, or greater than 1.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::{ChoiceId, QuestionAnswer};
    ///
    /// let yes = ChoiceId::try_from("yes")?;
    /// let answer = QuestionAnswer::new(yes.clone(), Some(0.9), None)?;
    /// assert_eq!(answer.choice(), &yes);
    /// assert!(QuestionAnswer::new(yes, Some(f64::NAN), None).is_err());
    /// # Ok::<(), sc_agent_types::JevError>(())
    /// ```
    pub fn new(
        choice: ChoiceId,
        confidence: Option<f64>,
        probabilities: Option<BTreeMap<ChoiceId, f64>>,
    ) -> Result<Self, JevError> {
        if let Some(value) = confidence {
            check_unit("confidence", &choice, value)?;
        }
        if let Some(map) = &probabilities {
            for value in map.values() {
                check_unit("probability", &choice, *value)?;
            }
        }
        Ok(Self {
            choice,
            confidence,
            probabilities,
        })
    }

    /// The selected choice.
    #[must_use]
    pub fn choice(&self) -> &ChoiceId {
        &self.choice
    }

    /// Backend confidence in [`Self::choice`]; `None` for LLM backends.
    #[must_use]
    pub fn confidence(&self) -> Option<f64> {
        self.confidence
    }

    /// Per-choice probabilities, when the backend reports them.
    #[must_use]
    pub fn probabilities(&self) -> Option<&BTreeMap<ChoiceId, f64>> {
        self.probabilities.as_ref()
    }

    /// The prior-state form of this answer for `question_id`.
    #[must_use]
    pub fn to_choice_answer(&self, question_id: &QuestionId) -> ChoiceAnswer {
        ChoiceAnswer {
            question_id: question_id.clone(),
            choice: self.choice.clone(),
        }
    }
}

fn check_unit(what: &str, choice: &ChoiceId, value: f64) -> Result<(), JevError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(JevError::response_invalid(format!(
            "{what} {value} for choice {choice} is not a finite number in [0, 1]"
        )))
    }
}

/// Normalized answer both backend families emit.
///
/// Built with [`Self::new`] against the request it answers, so every outcome
/// has exactly the request's question ids and only offered choices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JudgeOutcome {
    answers: BTreeMap<QuestionId, QuestionAnswer>,
    backend: BackendId,
    #[serde(default)]
    provider_meta: Option<Value>,
    #[serde(default)]
    input_sha256: Option<Sha256Digest>,
}

impl JudgeOutcome {
    /// Builds an outcome of `backend` for `request`.
    ///
    /// # Errors
    ///
    /// `RESPONSE_INVALID` when the answer keys differ from the request's
    /// question ids, or an answer's choice or probability key is not one of
    /// that question's choices.
    pub fn new(
        backend: BackendId,
        request: &JudgeRequest,
        answers: BTreeMap<QuestionId, QuestionAnswer>,
    ) -> Result<Self, JevError> {
        let asked = request.payload().question_choices();
        if !answers.keys().eq(asked.keys()) {
            return Err(JevError::response_invalid(
                "answer ids differ from the question ids",
            ));
        }
        for (id, answer) in &answers {
            let offered = &asked[id];
            if !offered.contains(answer.choice()) {
                return Err(JevError::response_invalid(format!(
                    "answer {id} choice {} is not one of its choices",
                    answer.choice()
                )));
            }
            if let Some(probabilities) = answer.probabilities()
                && let Some(extra) = probabilities.keys().find(|key| !offered.contains(*key))
            {
                return Err(JevError::response_invalid(format!(
                    "answer {id} has a probability for {extra}, which is not one of its choices"
                )));
            }
        }
        Ok(Self {
            answers,
            backend,
            provider_meta: None,
            input_sha256: None,
        })
    }

    /// Sets provider metadata (debugging only; not part of equal-input).
    #[must_use]
    pub fn with_provider_meta(mut self, meta: Value) -> Self {
        self.provider_meta = Some(meta);
        self
    }

    /// Sets the digest of the canonical payload the backend reports it sent.
    #[must_use]
    pub fn with_input_sha256(mut self, digest: Sha256Digest) -> Self {
        self.input_sha256 = Some(digest);
        self
    }

    /// Answers keyed by question id.
    #[must_use]
    pub fn answers(&self) -> &BTreeMap<QuestionId, QuestionAnswer> {
        &self.answers
    }

    /// Backend that produced the answers.
    #[must_use]
    pub fn backend(&self) -> BackendId {
        self.backend
    }

    /// Provider metadata, if any.
    #[must_use]
    pub fn provider_meta(&self) -> Option<&Value> {
        self.provider_meta.as_ref()
    }

    /// The reported input digest; `None` means "this backend does not attest".
    #[must_use]
    pub fn input_sha256(&self) -> Option<&Sha256Digest> {
        self.input_sha256.as_ref()
    }
}

/// Backend facts a caller may read before or after a backend exists.
///
/// Always derived from a [`BackendDescriptor`]; implementors do not restate
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendCapabilities {
    family: BackendFamily,
    key_id: Option<BackendKeyId>,
    requires_llm_context: bool,
    supports_session_ask: bool,
    request_limit_bytes: u64,
}

impl BackendCapabilities {
    /// Copies the facts of `descriptor`.
    #[must_use]
    pub const fn from_descriptor(descriptor: &BackendDescriptor) -> Self {
        Self {
            family: descriptor.family(),
            key_id: descriptor.key_id(),
            requires_llm_context: descriptor.requires_llm_context(),
            supports_session_ask: descriptor.supports_session_ask(),
            request_limit_bytes: descriptor.request_limit_bytes(),
        }
    }

    /// Backend family.
    #[must_use]
    pub const fn family(&self) -> BackendFamily {
        self.family
    }

    /// Credential the backend sends, if any.
    #[must_use]
    pub const fn key_id(&self) -> Option<BackendKeyId> {
        self.key_id
    }

    /// Whether requests need LLM context.
    #[must_use]
    pub const fn requires_llm_context(&self) -> bool {
        self.requires_llm_context
    }

    /// Whether sessions accept `ask`.
    #[must_use]
    pub const fn supports_session_ask(&self) -> bool {
        self.supports_session_ask
    }

    /// Largest wire request, in bytes.
    #[must_use]
    pub const fn request_limit_bytes(&self) -> u64 {
        self.request_limit_bytes
    }
}

/// Sends one canonical payload to a judge.
///
/// HTTP clients and API keys are construction state on the implementor.
/// The trait is **intentionally open** (not sealed): other crates may
/// implement it, and every method added after Phase A.1 r-1 has a default
/// body (ADR-SAG-0009, ADR-SAG-0027).
///
/// # Blocking
///
/// [`Self::validate_for_send`] and [`Self::judge`] block the calling thread
/// for the whole network exchange. Async hosts call them inside
/// `spawn_blocking` (or an equivalent blocking pool).
pub trait JudgeBackend: Send + Sync {
    /// Returns the backend id.
    fn id(&self) -> BackendId;

    /// Returns the backend's static facts, read from its descriptor.
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::from_descriptor(self.id().descriptor())
    }

    /// Refuses the request before any network send.
    ///
    /// The engine calls this once per send; `judge` does not repeat it.
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
    /// Returns [`JevError`] when the judge call fails or the response is
    /// invalid.
    fn judge(&self, req: &JudgeRequest) -> Result<JudgeOutcome, JevError>;
}

/// How a multi-backend run feeds earlier answers to later backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum MultiBackendMode {
    /// Every backend sees only the canonical payload and the system prompt.
    #[default]
    Independent,
    /// Later LLM backends also see earlier answers as session context.
    ChainContext,
}

/// Equal-input attestation of one backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Attestation {
    /// The backend reported the run's `prompt_sha256`.
    Matched,
    /// The backend reported a different digest.
    Mismatched {
        /// The digest the backend reported.
        reported: Sha256Digest,
    },
    /// The backend does not report a digest.
    Unattested,
    /// The send failed, so nothing was attested.
    Failed,
    /// Nothing was sent (dry-run or refusal).
    NotSent,
}

/// One backend's [`Attestation`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackendAttestation {
    backend: BackendId,
    attestation: Attestation,
}

impl BackendAttestation {
    /// Records `attestation` for `backend`.
    #[must_use]
    pub fn new(backend: BackendId, attestation: Attestation) -> Self {
        Self {
            backend,
            attestation,
        }
    }

    /// The backend.
    #[must_use]
    pub fn backend(&self) -> BackendId {
        self.backend
    }

    /// Its attestation.
    #[must_use]
    pub fn attestation(&self) -> &Attestation {
        &self.attestation
    }
}

/// Equal-input attestation for one check run.
///
/// `equal` is derived, not stored: [`Self::equal`] is true iff every backend
/// is [`Attestation::Matched`]. Serialization writes `equal` from that
/// method, and deserialization rejects a document whose `equal` differs from
/// the recomputed value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "EqualInputWire", into = "EqualInputWire")]
pub struct EqualInputRecord {
    prompt_sha256: Sha256Digest,
    mode: MultiBackendMode,
    backends: Vec<BackendAttestation>,
    leak: bool,
    producer: Option<SubjectsProducer>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EqualInputWire {
    prompt_sha256: Sha256Digest,
    mode: MultiBackendMode,
    backends: Vec<BackendAttestation>,
    leak: bool,
    equal: bool,
    producer: Option<SubjectsProducer>,
}

impl From<EqualInputRecord> for EqualInputWire {
    fn from(record: EqualInputRecord) -> Self {
        let equal = record.equal();
        Self {
            prompt_sha256: record.prompt_sha256,
            mode: record.mode,
            backends: record.backends,
            leak: record.leak,
            equal,
            producer: record.producer,
        }
    }
}

impl TryFrom<EqualInputWire> for EqualInputRecord {
    type Error = JevError;

    fn try_from(wire: EqualInputWire) -> Result<Self, Self::Error> {
        let record = Self::new(
            wire.prompt_sha256,
            wire.mode,
            wire.backends,
            wire.leak,
            wire.producer,
        );
        if record.equal() == wire.equal {
            Ok(record)
        } else {
            Err(JevError::input_invalid(
                "equal-input record field equal does not match its backend attestations",
            ))
        }
    }
}

impl EqualInputRecord {
    /// Builds a record.
    ///
    /// `leak` is true when a requested backend was withheld from the
    /// comparison. `producer` is the subjects producer when the check used a
    /// `subjects` or `command` input.
    #[must_use]
    pub fn new(
        prompt_sha256: Sha256Digest,
        mode: MultiBackendMode,
        backends: Vec<BackendAttestation>,
        leak: bool,
        producer: Option<SubjectsProducer>,
    ) -> Self {
        Self {
            prompt_sha256,
            mode,
            backends,
            leak,
            producer,
        }
    }

    /// SHA-256 of the canonical payload bytes.
    #[must_use]
    pub fn prompt_sha256(&self) -> &Sha256Digest {
        &self.prompt_sha256
    }

    /// The multi-backend mode of the run.
    #[must_use]
    pub fn mode(&self) -> MultiBackendMode {
        self.mode
    }

    /// Per-backend attestations, in run order.
    #[must_use]
    pub fn backends(&self) -> &[BackendAttestation] {
        &self.backends
    }

    /// Whether any backend was withheld from the comparison.
    #[must_use]
    pub fn leak(&self) -> bool {
        self.leak
    }

    /// Subjects producer, when the check used one.
    #[must_use]
    pub fn producer(&self) -> Option<&SubjectsProducer> {
        self.producer.as_ref()
    }

    /// True iff there is at least one backend and every one is `Matched`.
    #[must_use]
    pub fn equal(&self) -> bool {
        !self.backends.is_empty()
            && self
                .backends
                .iter()
                .all(|entry| matches!(entry.attestation, Attestation::Matched))
    }
}
