//! Test doubles for the judge contract (feature `test-util`).
//!
//! The one `JudgeBackend` double of the workspace (ARCHC-010). Enable the
//! `test-util` feature from a dev-dependency (or pass `--features test-util`)
//! to use it.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use serde_json::Value;

use crate::error::JevError;
use crate::ids::{BackendId, QuestionId, Sha256Digest};
use crate::judge::{JudgeBackend, JudgeOutcome, JudgeRequest, QuestionAnswer};

/// How a [`MockJudgeBackend`] ends a call.
#[derive(Debug, Clone)]
enum Behaviour {
    /// Answer from the script.
    Answer,
    /// `validate_for_send` passes, then `judge` returns the error (sent and failed).
    FailAfterSend(JevError),
    /// `validate_for_send` (and `judge`) returns the error; nothing is sent.
    Refuse(JevError),
}

/// In-process judge that answers from a script and records its requests.
///
/// Contract:
///
/// - the outcome is built at `judge` time with [`JudgeOutcome::new`] against
///   the request received, so answers must match its questions;
/// - every accepted request is recorded ([`Self::requests`]) and every
///   `validate_for_send` call is counted ([`Self::validations`]); the
///   counters use interior mutability, so a test that keeps an `Arc` and
///   registers a clone of it reads its own log;
/// - the outcome reports `input_sha256` = the digest of the bundle it
///   received, or the digest set by [`Self::with_reported_digest`];
/// - [`Self::with_failure`] makes `judge` fail after validation passed;
///   [`Self::refusing`] makes `validate_for_send` (and `judge`) fail, so
///   nothing is sent;
/// - when the id's descriptor requires LLM context and the request carries
///   none, `validate_for_send` (still counted) and `judge` return
///   `INPUT_INVALID` and record nothing (ADR-SAG-0022).
#[derive(Debug)]
pub struct MockJudgeBackend {
    id: BackendId,
    answers: BTreeMap<QuestionId, QuestionAnswer>,
    provider_meta: Option<Value>,
    reported_digest: Option<Sha256Digest>,
    behaviour: Behaviour,
    requests: Mutex<Vec<JudgeRequest>>,
    validations: AtomicUsize,
}

impl MockJudgeBackend {
    /// A judge for `id` that returns `answers` for every accepted request.
    #[must_use]
    pub fn new(id: BackendId, answers: BTreeMap<QuestionId, QuestionAnswer>) -> Self {
        Self {
            id,
            answers,
            provider_meta: None,
            reported_digest: None,
            behaviour: Behaviour::Answer,
            requests: Mutex::new(Vec::new()),
            validations: AtomicUsize::new(0),
        }
    }

    /// A judge for `id` whose `validate_for_send` returns `error`.
    #[must_use]
    pub fn refusing(id: BackendId, error: JevError) -> Self {
        let mut mock = Self::new(id, BTreeMap::new());
        mock.behaviour = Behaviour::Refuse(error);
        mock
    }

    /// Sets the outcome's `provider_meta`.
    #[must_use]
    pub fn with_provider_meta(mut self, meta: Value) -> Self {
        self.provider_meta = Some(meta);
        self
    }

    /// Reports `digest` as `input_sha256` instead of the received bundle's.
    #[must_use]
    pub fn with_reported_digest(mut self, digest: Sha256Digest) -> Self {
        self.reported_digest = Some(digest);
        self
    }

    /// Makes `judge` return `error` after `validate_for_send` passed.
    #[must_use]
    pub fn with_failure(mut self, error: JevError) -> Self {
        self.behaviour = Behaviour::FailAfterSend(error);
        self
    }

    /// Requests judged so far, in call order.
    #[must_use]
    pub fn requests(&self) -> Vec<JudgeRequest> {
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Number of `validate_for_send` calls.
    #[must_use]
    pub fn validations(&self) -> usize {
        self.validations.load(Ordering::SeqCst)
    }

    fn check(&self, request: &JudgeRequest) -> Result<(), JevError> {
        if self.id.descriptor().requires_llm_context() && request.llm().is_none() {
            return Err(JevError::input_invalid(format!(
                "backend {} requires an LLM system prompt",
                self.id
            )));
        }
        match &self.behaviour {
            Behaviour::Refuse(error) => Err(error.clone()),
            Behaviour::Answer | Behaviour::FailAfterSend(_) => Ok(()),
        }
    }
}

impl JudgeBackend for MockJudgeBackend {
    fn id(&self) -> BackendId {
        self.id
    }

    fn validate_for_send(&self, request: &JudgeRequest) -> Result<(), JevError> {
        self.validations.fetch_add(1, Ordering::SeqCst);
        self.check(request)
    }

    fn judge(&self, request: &JudgeRequest) -> Result<JudgeOutcome, JevError> {
        self.check(request)?;
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        if let Behaviour::FailAfterSend(error) = &self.behaviour {
            return Err(error.clone());
        }
        let digest = self
            .reported_digest
            .clone()
            .unwrap_or_else(|| request.payload().sha256().clone());
        let mut outcome =
            JudgeOutcome::new(self.id, request, self.answers.clone())?.with_input_sha256(digest);
        if let Some(meta) = &self.provider_meta {
            outcome = outcome.with_provider_meta(meta.clone());
        }
        Ok(outcome)
    }
}

/// Field names every serialized `JudgeOutcome` carries, sorted.
///
/// Contract tests in `sc-jev` and `sc-jev-backends` compare their outcomes'
/// serialized keys to this list so both backend families fill the same set.
#[must_use]
pub fn judge_outcome_field_names() -> &'static [&'static str] {
    &["answers", "backend", "input_sha256", "provider_meta"]
}
