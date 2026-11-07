//! TypeSafe Jev HTTP client.
//!
//! `sc-jev` owns every TypeSafe Jev HTTP operation (REQ-SAG-0110). Phase A
//! implements the check/judge slice: [`JevClient`] implements
//! [`JudgeBackend`](sc_agent_types::JudgeBackend), posts the canonical payload
//! byte-exact with the pinned model, and retries busy responses.
//!
//! The crate does not read `check.toml`, run extractors, or resolve API keys.
//! Callers pass a [`ResolvedApiKey`](sc_agent_types::ResolvedApiKey) and an
//! `sc_agent_http::Transport` at construction. HTTP, busy retry, the 8 MiB
//! response cap and redaction live in `sc-agent-http` (ADR-SAG-0026); this
//! crate owns the Jev wire body and the reply parser (ADR-SAG-0012).
//!
//! # Errors
//!
//! Failures use [`JevError`](sc_agent_types::JevError). Oversize input is
//! `INPUT_TOO_LARGE` and is not sent. Busy HTTP statuses (429, 502, 529)
//! retry under `sc_agent_http::BusyPolicy` (REQ-SAG-0505) before a
//! `NETWORK_ERROR` kind `Busy`. A reply that fails the answer contract is
//! `RESPONSE_INVALID`.

#![warn(missing_docs)]

mod check;
mod client;
mod config;

#[doc(inline)]
pub use client::JevClient;
#[doc(inline)]
pub use config::JevClientConfig;
