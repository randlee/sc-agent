//! In-process judges for tests (ADR-SAG-0022).
//!
//! [`ScriptedJudge`] returns a fixed answer object, records every request it
//! judges, counts pre-send validations, and never opens a socket.
//! [`register_scripted_backends`] builds a [`BackendRegistry`] of them.
//! Production builds leave the `test-util` feature off, so
//! [`crate::run::run_check`] stays the caller API (ADR-SAG-0009).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use sc_agent_types::{BackendId, JevError, JudgeBackend, JudgeOutcome, JudgeRequest};
use serde_json::Value;

use super::BackendRegistry;

/// Requests judged by one or more [`ScriptedJudge`] values, in call order.
///
/// Clones share one log, so several judges can record into it.
#[derive(Debug, Clone, Default)]
pub struct RequestLog(Arc<Mutex<Vec<JudgeRequest>>>);

impl RequestLog {
    /// Creates an empty log.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Copies the recorded requests.
    #[must_use]
    pub fn snapshot(&self) -> Vec<JudgeRequest> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn push(&self, request: &JudgeRequest) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
    }
}

/// A [`JudgeBackend`] that answers from a script and records its requests.
///
/// LLM ids ([`BackendId::is_llm`]) refuse a request without a system prompt,
/// like the real Haiku and Luna backends. Clones share the request log and
/// the validation counter.
///
/// # Examples
///
/// ```
/// use sc_agent_types::BackendId;
/// use sc_jev_check::registry::{BackendRegistry, ScriptedJudge};
///
/// let judge = ScriptedJudge::new(BackendId::Jev, serde_json::json!({"q": {"choice": "yes"}}));
/// let mut registry = BackendRegistry::new();
/// registry.register(std::sync::Arc::new(judge.clone())).unwrap();
/// assert!(judge.requests().is_empty());
/// ```
#[derive(Clone)]
pub struct ScriptedJudge {
    id: BackendId,
    answers: Arc<Value>,
    provider_meta: Option<Value>,
    requests: RequestLog,
    validations: Arc<AtomicUsize>,
}

impl ScriptedJudge {
    /// Judge for `id` that returns `answers` for every accepted request.
    #[must_use]
    pub fn new(id: BackendId, answers: Value) -> Self {
        Self::shared(id, Arc::new(answers))
    }

    fn shared(id: BackendId, answers: Arc<Value>) -> Self {
        Self {
            id,
            answers,
            provider_meta: None,
            requests: RequestLog::new(),
            validations: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Returns `meta` as [`JudgeOutcome::provider_meta`].
    #[must_use]
    pub fn with_provider_meta(mut self, meta: Value) -> Self {
        self.provider_meta = Some(meta);
        self
    }

    /// Records requests into `log` instead of a private log.
    #[must_use]
    pub fn with_request_log(mut self, log: &RequestLog) -> Self {
        self.requests = log.clone();
        self
    }

    /// Requests judged so far, in call order.
    #[must_use]
    pub fn requests(&self) -> Vec<JudgeRequest> {
        self.requests.snapshot()
    }

    /// Number of [`JudgeBackend::validate_for_send`] calls from callers.
    ///
    /// The check `judge` repeats internally is not counted.
    #[must_use]
    pub fn validations(&self) -> usize {
        self.validations.load(Ordering::Relaxed)
    }

    fn check(&self, request: &JudgeRequest) -> Result<(), JevError> {
        if self.id.is_llm() && request.llm.is_none() {
            return Err(JevError::InputInvalid {
                detail: "LLM system prompt is required".into(),
            });
        }
        Ok(())
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
        self.validations.fetch_add(1, Ordering::Relaxed);
        self.check(request)
    }

    fn judge(&self, request: &JudgeRequest) -> Result<JudgeOutcome, JevError> {
        self.check(request)?;
        self.requests.push(request);
        Ok(JudgeOutcome {
            answers: self.answers.as_ref().clone(),
            backend: self.id,
            probabilities: None,
            provider_meta: self.provider_meta.clone(),
        })
    }
}

/// Registers one scripted judge per `ids` entry.
///
/// `answers` is a JSON object keyed by question id. Each value is either an
/// answer object (`{"choice": "yes"}`) or the string shorthand `"yes"`, which
/// is stored as `{"choice": "yes"}`. Every caller (CLI tests, the Python
/// binding's `_testing` module) therefore accepts the same shape.
///
/// Each judge returns the normalised answers and does not open a socket or
/// store an API key. Callers pass the registry to
/// [`crate::run::RunOptions::registry`]. This does not change
/// [`crate::run::run_check`]'s parameters (ADR-SAG-0009).
///
/// # Errors
///
/// Returns [`JevError::InputInvalid`] when `answers` is not an object or a
/// value is neither a string nor an object, and
/// [`JevError::registry_invalid`] when `ids` repeats a backend.
///
/// # Examples
///
/// ```
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
    for judge in shared_judges(ids, answers) {
        registry.register(Arc::new(judge))?;
    }
    Ok(registry)
}

/// Expands the `"choice"` string shorthand into answer objects.
fn normalize_answers(answers: Value) -> Result<Value, JevError> {
    let Value::Object(map) = answers else {
        return Err(JevError::InputInvalid {
            detail: "scripted answers must be a JSON object of question ids".into(),
        });
    };
    let mut normalized = serde_json::Map::new();
    for (id, answer) in map {
        let wrapped = match answer {
            Value::String(choice) => serde_json::json!({ "choice": choice }),
            object @ Value::Object(_) => object,
            _ => {
                return Err(JevError::InputInvalid {
                    detail: format!("scripted answer for {id} must be a string or object"),
                });
            }
        };
        normalized.insert(id, wrapped);
    }
    Ok(Value::Object(normalized))
}

fn shared_judges(ids: &[BackendId], answers: Value) -> Vec<ScriptedJudge> {
    let shared = Arc::new(answers);
    ids.iter()
        .copied()
        .map(|id| ScriptedJudge::shared(id, Arc::clone(&shared)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_agent_types::PayloadBundle;

    #[test]
    fn string_shorthand_and_object_answers_normalise_alike() {
        let shorthand = normalize_answers(serde_json::json!({"q": "yes"})).expect("shorthand");
        let object = normalize_answers(serde_json::json!({"q": {"choice": "yes"}})).expect("obj");
        assert_eq!(shorthand, object);
        assert_eq!(shorthand, serde_json::json!({"q": {"choice": "yes"}}));
        for bad in [serde_json::json!("yes"), serde_json::json!({"q": 1})] {
            let error = normalize_answers(bad).expect_err("bad shape");
            assert_eq!(error.code(), "INPUT_INVALID");
        }
    }

    #[test]
    fn scripted_answers_share_one_allocation() {
        let judges = shared_judges(
            &[BackendId::Jev, BackendId::OpenAiLuna],
            serde_json::json!({"relevant": "yes"}),
        );
        assert_eq!(judges.len(), 2);
        assert!(Arc::ptr_eq(&judges[0].answers, &judges[1].answers));
    }

    #[test]
    fn scripted_judge_records_requests_and_counts_validations() {
        let log = RequestLog::new();
        let judge = ScriptedJudge::new(BackendId::AnthropicHaiku, serde_json::json!({}))
            .with_request_log(&log);
        let bare = JudgeRequest {
            payload: PayloadBundle::from_canonical_bytes(b"{}".to_vec()),
            llm: None,
        };
        assert_eq!(
            judge.validate_for_send(&bare).expect_err("llm").code(),
            "INPUT_INVALID"
        );
        assert!(judge.judge(&bare).is_err());
        assert_eq!(judge.validations(), 1);
        assert!(log.snapshot().is_empty());
    }
}
