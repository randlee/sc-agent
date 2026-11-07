//! API-direct Haiku and Luna [`JudgeBackend`](sc_agent_types::JudgeBackend)s.
//!
//! Callers pass the evaluator system prompt on [`sc_agent_types::JudgeRequest::llm`].
//! This crate embeds that text and the canonical payload bytes in the provider
//! body. It does not read `check.toml`, resolve secrets, or embed evaluator prose.
//! `prompt_sha256` stays the digest of the canonical payload alone (ADR-SAG-0019).
//!
//! The `sc_agent_http::Transport` and the resolved API key are construction
//! state (ADR-SAG-0006). HTTP, busy retry, the response cap and redaction
//! live in `sc-agent-http` (ADR-SAG-0026); this crate owns the provider wire
//! bodies and reply parsers (ADR-SAG-0019). `with_default_transport` uses
//! `sc_agent_http::UreqTransport` with connect 10 s and request 120 s
//! (ADR-SAG-0007). Busy statuses 429, 502 and 529 are retried with the same
//! body under `sc_agent_http::BusyPolicy` (REQ-SAG-0505).

#![warn(missing_docs)]

mod backend;
mod validate;
mod wire;

#[doc(inline)]
pub use backend::{AnthropicHaikuBackend, OpenAiLunaBackend};
