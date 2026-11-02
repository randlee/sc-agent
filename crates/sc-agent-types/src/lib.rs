//! Shared types for sc-agent crates.
//!
//! # Errors
//!
//! [`JevError`] is the only CLI and PyO3 error-id source (ADR-SAG-0001).
//! Each variant exposes [`JevError::code`] and [`JevError::fix_hint`].
//! The stderr JSON shape is [`ScAgentErrorResponse`].
//!
//! # Canonical judge payload
//!
//! ADR-SAG-0002: payload bytes are sorted-key compact JSON, encoded as UTF-8.
//! Object keys are stored in a [`std::collections::BTreeMap`] (`serde_json::Map`
//! without `preserve_order`). [`canonical_json_bytes`] re-sorts keys and emits
//! `serde_json`'s compact encoding. Non-ASCII characters stay UTF-8. SHA-256
//! of those bytes is `prompt_sha256`. LLM system-prompt
//! text is not part of the digest (ADR-SAG-0019).
//!
//! # Observability
//!
//! [`observability::v2`] is a temporary `SessionLog` port. It depends on
//! `sc-observability-types` 1.5.0 and is removed by bead
//! `agt-obs-v2-shim-removal`. This crate does not link the full
//! `sc-observability` runtime (ADR-SAG-0016).

#![warn(missing_docs)]

mod envelope;
mod error;
mod ids;
mod judge;
mod payload;
mod subjects;

pub mod observability;

#[cfg(any(test, feature = "test-util"))]
pub mod test_util;

#[doc(inline)]
pub use envelope::ScAgentErrorResponse;
#[doc(inline)]
pub use error::JevError;
#[doc(inline)]
pub use ids::{BackendId, BackendKeyId, RepoRoot, Rev, SessionId, Sha256Digest};
#[doc(inline)]
pub use judge::{
    CanonicalJudgePayload, ChoiceAnswer, EqualInputRecord, JudgeBackend, JudgeOutcome,
    JudgeRequest, LlmWireContext, PayloadBundle, Question,
};
#[doc(inline)]
pub use payload::{canonical_json_bytes, payload_bundle_from_value};
#[doc(inline)]
pub use subjects::{
    InputSpec, SUBJECTS_SCHEMA_V1, Span, Subject, SubjectsDocument, SubjectsProducer,
    SubjectsSource,
};
