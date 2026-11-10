//! Sync HTTP layer of sc-agent: one send per call, no queue, no sleep.
//!
//! - [`HttpRequest`] / [`HttpResponse`] carry byte bodies (multipart works);
//!   [`Endpoint`] accepts https, or http only for a configured local runner.
//! - [`HttpError`] is the one error type, with stable `http.*` ids
//!   ([`ErrorId`](sc_agent_types::ErrorId), REQ-SAG-0002).
//!
//! It sits above sc-agent-types, sc-agent-secrets and sc-agent-log and below
//! sc-decision-schema and sc-llm-provider (ADR-SAG-0004); it starts no
//! runtime and never sleeps (REQ-SAG-0009, REQ-SAG-0010).
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;
mod quota;
mod reply;
mod request;
mod resend;
mod retry_after;

#[doc(inline)]
pub use error::{BusyReason, HttpError, NetworkKind};
#[doc(inline)]
pub use reply::ProviderReply;
#[doc(inline)]
pub use request::{Endpoint, HttpRequest, HttpResponse, Method, SecretHeader};
/// Which request a log event belongs to; re-exported because `send_once`
/// takes it.
pub use sc_agent_log::LogContext;
