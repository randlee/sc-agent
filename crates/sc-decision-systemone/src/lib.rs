//! System One decision backend of sc-agent: [`SystemOneBackend`].
//!
//! One sans-IO [`Backend`](sc_decision_schema::Backend) for every System One server: TypeSafe
//! Jev ([`TYPESAFE_BASE_URL`], Bearer key) and the Jev-compatible local daemons (Strands
//! Decider, Laya, Mapika/decider; key optional). All of them take the same
//! [`SystemOneRequest`](sc_decision_schema::SystemOneRequest) unchanged and answer the same
//! [`SystemOneResponse`](sc_decision_schema::SystemOneResponse) (ADR-SAG-0001); there is no
//! backend-specific request type.
//!
//! - [`Backend::encode`](sc_decision_schema::Backend::encode) builds `POST {base}/v1/systemone`:
//!   JSON, or multipart/form-data when the request carries image parts, with the TypeSafe
//!   danger-zone headers and the Bearer key attached through
//!   [`HttpRequest::with_secret`](sc_agent_http::HttpRequest::with_secret).
//! - [`Backend::decode`](sc_decision_schema::Backend::decode) turns the final reply into a
//!   response or the matching [`DecisionError`](sc_decision_schema::DecisionError).
//! - [`SystemOneBackend::models_request`] / [`SystemOneBackend::decode_models`]: `GET
//!   {base}/v1/models`.
//!
//! The crate does no I/O and writes no log: the caller sends through sc-agent-http, which logs
//! the send. Busy replies (429/502/529) are handled by sc-agent-http and the runner before
//! decode; a 503 is a final reply here (REQ-SAG-0601). It has no error type of its own: every
//! function returns the trait-fixed `DecisionError` (ADR-SAG-0004 exemption). It sits above
//! sc-decision-schema and below sc-decision-runner, starts no runtime and never blocks a thread on a timer.
//!
//! # Examples
//!
//! ```
//! use sc_agent_http::{HttpResponse, LogContext, Method};
//! use sc_decision_schema::{Backend, DecisionRequest, SystemOneRequest};
//! use sc_decision_systemone::{SystemOneBackend, TYPESAFE_BASE_URL};
//!
//! let backend = SystemOneBackend::new(TYPESAFE_BASE_URL, None)?;
//! let body = SystemOneRequest::from_json(
//!     r#"{"model":"jev-latest","state":"hi","questions":{"q":{"type":"noul"}}}"#,
//! )?;
//! let request = DecisionRequest::try_from(body)?;
//! let http = backend.encode(&request)?;
//! assert_eq!(http.method(), Method::Post);
//! assert_eq!(http.endpoint().as_str(), "https://api.typesafe.ai/v1/systemone");
//!
//! let reply = HttpResponse::new(
//!     200,
//!     vec![],
//!     br#"{"model":"jev-latest","answers":{"q":{"type":"noul","noul":0.5}},"usage":{"input_tokens":1,"output_tokens":1}}"#.to_vec(),
//! );
//! let response = backend.decode(reply, &request, &LogContext::NONE)?;
//! assert_eq!(response.model, "jev-latest");
//! # Ok::<(), sc_decision_schema::DecisionError>(())
//! ```
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod backend;
mod multipart;

#[doc(inline)]
pub use backend::{SystemOneBackend, TYPESAFE_BASE_URL};
