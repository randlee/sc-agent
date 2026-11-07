//! API-direct Haiku and Luna [`JudgeBackend`](sc_agent_types::JudgeBackend)s.
//!
//! Callers pass the evaluator system prompt on [`sc_agent_types::JudgeRequest::llm`].
//! This crate embeds that text and the canonical payload bytes in the provider
//! body. It does not read `check.toml`, resolve secrets, or embed evaluator prose.
//! `prompt_sha256` stays the digest of the canonical payload alone (ADR-SAG-0019).
//!
//! HTTP transport and the resolved API key are construction state (ADR-SAG-0006).
//! Connect timeout is 10s and each provider call's read/write timeout is 120s
//! (ADR-SAG-0007); `with_timeouts` on either backend overrides both. Busy
//! statuses 429, 502, and 529 are retried with the same body, honoring
//! `Retry-After` (0 to 30 s plus jitter) within a 90 s budget per call.

#![warn(missing_docs)]

mod backend;
mod retry;
mod retry_policy;
#[cfg(test)]
mod test_server;
mod transport;
mod validate;
mod wire;

#[doc(inline)]
pub use backend::{AnthropicHaikuBackend, OpenAiLunaBackend};
#[doc(inline)]
pub use retry::RetrySleeper;
#[doc(inline)]
pub use transport::{
    BackendTransport, DEFAULT_CONNECT_TIMEOUT, DEFAULT_REQUEST_TIMEOUT, HttpRequest, HttpResponse,
    UreqTransport,
};
