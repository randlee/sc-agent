//! TypeSafe Jev HTTP client.
//!
//! `sc-jev` owns every TypeSafe Jev HTTP operation (REQ-SAG-0110). Phase A
//! implements the check/judge slice: [`JevClient`] implements
//! [`JudgeBackend`](sc_agent_types::JudgeBackend), posts the canonical payload
//! byte-exact with the pinned model, and retries busy responses.
//!
//! The crate does not read `check.toml`, run extractors, or resolve API keys.
//! Callers pass a [`ResolvedApiKey`](sc_agent_types::ResolvedApiKey) and a
//! [`JevTransport`] at construction. Only the transport reads the key.
//!
//! # Errors
//!
//! Failures use [`JevError`](sc_agent_types::JevError). Oversize input is
//! `INPUT_TOO_LARGE` and is not sent. Busy HTTP statuses retry before a
//! `NETWORK_ERROR`. A reply that fails the answer contract is
//! `RESPONSE_INVALID`.

#![warn(missing_docs)]

pub mod api;
pub mod client;

mod check;

#[doc(inline)]
pub use api::JevClientConfig;
#[doc(inline)]
pub use client::{
    HttpsTransport, JevClient, JevTransport, RetryDelay, ThreadRetryDelay, WireRequest,
    WireResponse,
};
