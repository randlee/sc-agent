//! Shared types for sc-agent crates.
//!
//! # Errors
//!
//! [`JevError`] is the only CLI and PyO3 error-id source (ADR-SAG-0001,
//! ADR-SAG-0027). [`JevError::code`] returns an [`ErrorCode`];
//! [`JevError::fix_hint`] is the one renderer of recovery text. The stderr
//! JSON shape is [`ScAgentErrorResponse`].
//!
//! # Backends
//!
//! [`BACKENDS`] is the static fact table; [`BackendId::descriptor`] reads it.
//! Code outside this crate branches on descriptor facts, never on
//! `BackendId` equality (ADR-SAG-0009 r-1 amendment).
//!
//! # Canonical judge payload
//!
//! ADR-SAG-0002: payload bytes are sorted-key compact JSON, encoded as UTF-8.
//! `PayloadBundle::encode` builds every bundle; SHA-256 of the bytes is
//! `prompt_sha256`. LLM system-prompt text is not part of the digest
//! (ADR-SAG-0019).
//!
//! # Cargo features (module seams)
//!
//! | Feature | Default | Gates |
//! | --- | --- | --- |
//! | (core) | always | `error`, `envelope`, `ids`, [`schema`], `cancel` |
//! | `judge` | yes | the judge contract and canonical payload |
//! | `secret` | yes | `ResolvedApiKey` |
//! | `obs` | yes | `observability::v2` (session log port) |
//! | `git` | no | `git` (argv-only git helpers) |
//! | `test-util` | no | `test_util::MockJudgeBackend` (requires `judge`) |
//!
//! This crate links no `sc-observability*`, HTTP, Python, or tree-sitter
//! crate (REQ-SAG-0107).
//!
//! # Tests and fixtures
//!
//! Integration tests read `tests/fixtures/jev/**` at the repo root; they are
//! not part of the published crate package.

#![warn(missing_docs)]

mod cancel;
mod catalog;
mod code;
mod envelope;
mod error;
mod ids;
pub mod schema;

#[cfg(feature = "judge")]
mod judge;
#[cfg(feature = "judge")]
mod payload;
#[cfg(feature = "secret")]
mod secret;

#[cfg(feature = "git")]
pub mod git;
#[cfg(feature = "obs")]
pub mod observability;
#[cfg(feature = "test-util")]
pub mod test_util;

#[doc(inline)]
pub use cancel::CancelToken;
#[doc(inline)]
pub use catalog::{BACKENDS, BackendDescriptor, BackendFamily};
#[doc(inline)]
pub use envelope::{ERROR_RESPONSE_SCHEMA_VERSION, ScAgentErrorResponse};
#[doc(inline)]
pub use error::{ErrorCode, JevError, NetworkErrorKind, OperationalArea};
#[doc(inline)]
pub use ids::{
    BackendId, BackendKeyId, BackendSet, BlobSha, ChoiceId, Operation, QuestionId,
    RepoRelativePath, RepoRoot, Rev, SessionId, Sha256Digest,
};
#[cfg(feature = "judge")]
#[doc(inline)]
pub use judge::{
    Attestation, BackendAttestation, BackendCapabilities, EqualInputRecord, JudgeBackend,
    JudgeOutcome, JudgeRequest, LlmWireContext, MultiBackendMode, QuestionAnswer,
};
#[cfg(feature = "judge")]
#[doc(inline)]
pub use payload::{
    CanonicalJudgePayload, ChoiceAnswer, PayloadBundle, Question, canonical_json_bytes,
};
#[cfg(feature = "secret")]
#[doc(inline)]
pub use secret::ResolvedApiKey;
