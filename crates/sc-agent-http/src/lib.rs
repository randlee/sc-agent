//! Sync HTTP layer of sc-agent: one send per call, no queue, no sleep.
//!
//! - [`HttpRequest`] / [`HttpResponse`] carry byte bodies (multipart works);
//!   [`Endpoint`] accepts https, or http only for a configured local runner,
//!   and a key only over https or to a loopback host (REQ-SAG-0013).
//! - [`Transport`] sends one request once; [`UreqTransport`] is the
//!   production transport, bounded by [`Timeouts`].
//! - [`HttpClient`] admits a send ([`HttpClient::may_start`] /
//!   [`HttpClient::may_resend`]), makes it ([`HttpClient::send_once`]) and
//!   classifies the reply: final ([`Next::Done`], decoded by the backend),
//!   busy or timed out ([`Next::ResendAt`], a not-before instant for the
//!   resend under [`ResendPolicy`]), or quota ([`HttpError::QuotaExhausted`]).
//!   A busy reply pauses the host for every clone of the client
//!   ([`ProviderPause`], REQ-SAG-0012).
//! - [`HttpError`] is the one error type, with stable `http.*` ids
//!   ([`ErrorId`](sc_agent_types::ErrorId), REQ-SAG-0002); every error and log
//!   event is free of the key.
//!
//! The crate never waits and takes no `CancelToken`: the holdoff wait and
//! cancellation belong to the caller's queue. It sits above sc-agent-types,
//! sc-agent-secrets and sc-agent-log and below sc-decision-schema and
//! sc-llm-provider (ADR-SAG-0004); it starts no runtime and never sleeps
//! (REQ-SAG-0009, REQ-SAG-0010).
//!
//! Feature `testing` (not default) adds [`testing`]: transport test doubles
//! for consumers' tests.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;
mod quota;
mod redact;
mod reply;
mod request;
mod resend;
mod retry_after;
#[cfg(test)]
mod test_key;
mod transport;
mod ureq_transport;

#[doc(inline)]
pub use error::{BusyReason, HttpError, NetworkKind};
#[doc(inline)]
pub use reply::ProviderReply;
#[doc(inline)]
pub use request::{Endpoint, HttpRequest, HttpResponse, Method, SecretHeader};
#[doc(inline)]
pub use resend::{
    Attempts, HttpClient, Jitter, Next, Pending, ProviderPause, ResendPolicy, SendPermit, Start,
};
/// Which request a log event belongs to; re-exported because
/// [`HttpClient::send_once`] takes it.
pub use sc_agent_log::LogContext;
#[doc(inline)]
pub use transport::{Timeouts, Transport};
#[doc(inline)]
pub use ureq_transport::UreqTransport;

#[cfg(feature = "testing")]
pub mod testing;
