//! Shared blocking HTTP layer for sc-agent backends (ADR-SAG-0026, HTTP half).
//!
//! `sc-agent-http` carries bytes. It owns the request and response types, the
//! [`Transport`] trait, the production [`UreqTransport`], busy retry
//! ([`BusyPolicy`], [`send_with_busy_retry`]) with a cancellation hook, the
//! 8 MiB response cap, and redaction ([`redact()`]). The wire format of every
//! API stays in the backend crate that speaks it: `sc-jev` owns every Jev API
//! operation (ADR-SAG-0012) and `sc-jev-backends` the Haiku and Luna bodies.
//!
//! # Secrets
//!
//! A credential travels only as a [`SecretHeader`] holding a
//! [`ResolvedApiKey`](sc_agent_types::ResolvedApiKey) inside an
//! [`HttpRequest`]. The one place in the workspace that reads the key bytes
//! is the private `ureq_transport` module, when it writes the header
//! (ADR-SAG-0020 r-3 amendment). That is a lint-enforced convention, not a
//! type seal: `clippy.toml` lists `ResolvedApiKey::as_str` under
//! `disallowed-methods` and the one call site carries an `#[expect]`.
//!
//! # Retry and redaction
//!
//! Production code sends through [`send_and_parse`] (or
//! [`HttpClient::send_and_parse`]), which retries busy statuses under
//! [`BusyPolicy`] via [`send_with_busy_retry`] and passes every error it
//! returns, reply-parse errors included, through [`redact()`]. Calling
//! [`Transport::send`] directly bypasses both.
//!
//! # Cargo features
//!
//! | Feature | Default | Adds |
//! | --- | --- | --- |
//! | `test-util` | no | `BusyPolicy::with_test_hooks`, `Endpoint::loopback_http`, `UreqTransport::loopback_for_tests`, `test_util` |
//!
//! `test-util` is for dev-dependencies only. Even when it is on, plain HTTP
//! can reach only `127.0.0.1`.
//!
//! No type of the HTTP client crate (ureq) appears in any public signature.
//!
//! # Examples
//!
//! ```
//! use std::sync::Arc;
//!
//! use sc_agent_http::{
//!     BusyPolicy, Endpoint, HeaderName, HttpRequest, HttpResponse, Method, Transport,
//!     send_with_busy_retry,
//! };
//! use sc_agent_types::{CancelToken, JevError};
//!
//! struct Always200;
//!
//! impl Transport for Always200 {
//!     fn send(&self, _request: &HttpRequest) -> Result<HttpResponse, JevError> {
//!         Ok(HttpResponse::new(200, Vec::new(), b"{}".to_vec()))
//!     }
//! }
//!
//! let request = HttpRequest::new(
//!     Method::Post,
//!     Endpoint::parse("https://api.example.test/v1/x")?,
//!     Arc::from(&b"{}"[..]),
//! )
//! .with_header(HeaderName::new("content-type"), "application/json");
//! let response = send_with_busy_retry(
//!     &Always200,
//!     &request,
//!     &BusyPolicy::default(),
//!     &CancelToken::new(),
//! )?;
//! assert_eq!(response.status(), 200);
//! # Ok::<(), JevError>(())
//! ```

#![warn(missing_docs)]

mod redact;
mod request;
mod retry;
mod transport;
mod ureq_transport;

#[cfg(feature = "test-util")]
pub mod test_util;

#[doc(inline)]
pub use redact::redact;
#[doc(inline)]
pub use request::{Endpoint, HeaderName, HttpRequest, HttpResponse, Method, SecretHeader};
#[doc(inline)]
pub use retry::{BusyPolicy, HttpClient, send_and_parse, send_with_busy_retry};
#[doc(inline)]
pub use transport::{Timeouts, Transport};
#[doc(inline)]
pub use ureq_transport::UreqTransport;
