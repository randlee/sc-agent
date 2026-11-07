//! Canonical judge payload bytes (ADR-SAG-0002).
//!
//! # Encoding
//!
//! The canonical payload is compact JSON encoded as UTF-8:
//!
//! - Object keys are emitted in lexicographic byte order. The guarantee is
//!   [`canonical_json_bytes`]'s recursive re-sort: it walks every object and
//!   reinserts keys in sorted order before encoding. It does not depend on
//!   which map [`serde_json::Map`] is, so the bytes stay the same even if
//!   Cargo feature unification turns on `serde_json/preserve_order`.
//! - Strings use `serde_json`'s default escapes (`\"`, `\\`, and `\u00XX` for
//!   controls). Other characters are raw UTF-8, not `\u` escapes.
//! - Arrays keep their element order. Numbers, booleans, and null use
//!   `serde_json`'s compact forms. No extra whitespace is emitted.
//!
//! `prompt_sha256` is SHA-256 of those bytes. LLM system-prompt text is not
//! included.
//!
//! Every [`PayloadBundle`] is built by [`PayloadBundle::encode`] (or
//! [`PayloadBundle::decode`], which re-encodes and compares), so every bundle
//! carries the question and choice map of the payload it encodes.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::JevError;
use crate::ids::{ChoiceId, QuestionId, Sha256Digest};

/// One question the judge answers.
///
/// Fields are private so a later field is additive; build with
/// [`Self::new`] or deserialize.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Question {
    id: QuestionId,
    prompt: String,
    choices: Vec<ChoiceId>,
}

impl Question {
    /// A question with `id`, instructions `prompt`, and `choices` in display order.
    #[must_use]
    pub fn new(id: QuestionId, prompt: impl Into<String>, choices: Vec<ChoiceId>) -> Self {
        Self {
            id,
            prompt: prompt.into(),
            choices,
        }
    }

    /// The question id.
    #[must_use]
    pub fn id(&self) -> &QuestionId {
        &self.id
    }

    /// The instructions sent with the question.
    #[must_use]
    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    /// Replaces the instructions (input substitution).
    #[must_use]
    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = prompt.into();
        self
    }

    /// Choice labels, in display order.
    #[must_use]
    pub fn choices(&self) -> &[ChoiceId] {
        &self.choices
    }
}

/// A recorded choice for one question: prior state inside the payload.
///
/// This is an *input* (part of the `prompt_sha256` bytes). A backend's output
/// for one question is `QuestionAnswer`; `QuestionAnswer::to_choice_answer`
/// is the one conversion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChoiceAnswer {
    /// The question this answer belongs to.
    pub question_id: QuestionId,
    /// Selected choice label.
    pub choice: ChoiceId,
}

/// Questions plus state, before canonical encoding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalJudgePayload {
    questions: Vec<Question>,
    state: Vec<ChoiceAnswer>,
}

impl CanonicalJudgePayload {
    /// A payload of `questions` with prior `state`.
    #[must_use]
    pub fn new(questions: Vec<Question>, state: Vec<ChoiceAnswer>) -> Self {
        Self { questions, state }
    }

    /// Questions the backends must judge.
    #[must_use]
    pub fn questions(&self) -> &[Question] {
        &self.questions
    }

    /// Prior answers carried into this request.
    #[must_use]
    pub fn state(&self) -> &[ChoiceAnswer] {
        &self.state
    }
}

/// Canonical judge payload bytes, their SHA-256, and the questions they ask.
///
/// [`Self::sha256`] is computed on [`Self::bytes`] only. LLM system-prompt
/// text is not part of the digest (ADR-SAG-0019). Fields are private so the
/// digest and the question map always match the bytes; equal-input
/// attestation and `JudgeOutcome::new` rely on that invariant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadBundle {
    bytes: Vec<u8>,
    sha256: Sha256Digest,
    question_choices: BTreeMap<QuestionId, BTreeSet<ChoiceId>>,
}

impl PayloadBundle {
    /// Encodes `payload` as canonical bytes and records its digest.
    ///
    /// # Errors
    ///
    /// `INPUT_INVALID` when two questions share an id or `serde_json` cannot
    /// encode the payload.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::{CanonicalJudgePayload, PayloadBundle};
    ///
    /// let payload: CanonicalJudgePayload = serde_json::from_value(serde_json::json!({
    ///     "questions": [{"id": "q", "prompt": "Is it?", "choices": ["no", "yes"]}],
    ///     "state": []
    /// }))?;
    /// let bundle = PayloadBundle::encode(&payload)?;
    /// assert_eq!(
    ///     bundle.bytes(),
    ///     br#"{"questions":[{"choices":["no","yes"],"id":"q","prompt":"Is it?"}],"state":[]}"#
    /// );
    /// assert_eq!(bundle.question_ids().count(), 1);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn encode(payload: &CanonicalJudgePayload) -> Result<Self, JevError> {
        let mut question_choices = BTreeMap::new();
        for question in payload.questions() {
            let choices: BTreeSet<ChoiceId> = question.choices().iter().cloned().collect();
            if question_choices
                .insert(question.id().clone(), choices)
                .is_some()
            {
                return Err(JevError::input_invalid(format!(
                    "question id {} is duplicated",
                    question.id()
                )));
            }
        }
        let value = serde_json::to_value(payload)
            .map_err(|error| JevError::encode_failed("canonical payload", &error))?;
        let bytes = canonical_json_bytes(&value)?;
        let sha256 = Sha256Digest::of(&bytes);
        Ok(Self {
            bytes,
            sha256,
            question_choices,
        })
    }

    /// Rebuilds a bundle from bytes that must already be canonical.
    ///
    /// The bytes are parsed into a [`CanonicalJudgePayload`] and re-encoded;
    /// the result must equal `bytes`. Used to reload stored payloads.
    ///
    /// # Errors
    ///
    /// `INPUT_INVALID` when the bytes are not a canonical payload.
    pub fn decode(bytes: &[u8]) -> Result<Self, JevError> {
        let payload: CanonicalJudgePayload = serde_json::from_slice(bytes)
            .map_err(|error| JevError::encode_failed("canonical payload", &error))?;
        let bundle = Self::encode(&payload)?;
        if bundle.bytes != bytes {
            return Err(JevError::input_invalid(
                "payload bytes are not canonical sorted-key compact JSON",
            ));
        }
        Ok(bundle)
    }

    /// Sorted-key compact JSON, UTF-8.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// SHA-256 of [`Self::bytes`].
    #[must_use]
    pub fn sha256(&self) -> &Sha256Digest {
        &self.sha256
    }

    /// Each question id and the set of its choices, computed by [`Self::encode`].
    #[must_use]
    pub fn question_choices(&self) -> &BTreeMap<QuestionId, BTreeSet<ChoiceId>> {
        &self.question_choices
    }

    /// The question ids, in sorted order.
    pub fn question_ids(&self) -> impl Iterator<Item = &QuestionId> + '_ {
        self.question_choices.keys()
    }

    /// Returns the bytes and their digest, consuming the bundle.
    #[must_use]
    pub fn into_parts(self) -> (Vec<u8>, Sha256Digest) {
        (self.bytes, self.sha256)
    }
}

/// Encodes `value` as canonical judge-payload bytes.
///
/// # Errors
///
/// Returns `INPUT_INVALID` when `serde_json` cannot encode `value`.
///
/// # Examples
///
/// ```
/// let value = serde_json::json!({"b": 1, "a": {"d": true, "c": "ok"}});
/// let bytes = sc_agent_types::canonical_json_bytes(&value).expect("finite JSON");
/// assert_eq!(bytes, br#"{"a":{"c":"ok","d":true},"b":1}"#);
/// ```
pub fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>, JevError> {
    serde_json::to_vec(&sorted(value))
        .map_err(|error| JevError::encode_failed("canonical JSON", &error))
}

fn sorted(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|left, right| left.0.cmp(right.0));
            let mut ordered = Map::new();
            for (key, child) in entries {
                ordered.insert(key.clone(), sorted(child));
            }
            Value::Object(ordered)
        }
        Value::Array(items) => Value::Array(items.iter().map(sorted).collect()),
        other => other.clone(),
    }
}
