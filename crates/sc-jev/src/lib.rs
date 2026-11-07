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
//! `INPUT_TOO_LARGE` and is not sent. Busy HTTP statuses (429, 502, 529)
//! retry with `Retry-After` clamped to 30 s plus jitter, within a 90 s budget,
//! before a `NETWORK_ERROR`. A reply that fails the answer contract is
//! `RESPONSE_INVALID`.

#![warn(missing_docs)]

mod check;
mod client;
mod config;
mod retry_policy;
#[cfg(test)]
mod test_server;


#[doc(inline)]
pub use client::{
    HttpsTransport, JevClient, JevTransport, RetryDelay, ThreadRetryDelay, WireRequest,
    WireResponse,
};
#[doc(inline)]
pub use config::JevClientConfig;

/// The verbatim copies shared with `sc-jev-backends` must stay identical.
#[cfg(test)]
mod shared_copies {
    #[test]
    fn retry_policy_and_test_server_match_the_sc_jev_backends_copies() {
        assert_eq!(
            include_str!("retry_policy.rs"),
            include_str!("../../sc-jev-backends/src/retry_policy.rs"),
            "retry_policy.rs copies drifted; edit both together"
        );
        assert_eq!(
            include_str!("test_server.rs"),
            include_str!("../../sc-jev-backends/src/test_server.rs"),
            "test_server.rs copies drifted; edit both together"
        );
    }
}
