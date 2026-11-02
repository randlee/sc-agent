//! Injectable HTTP transport (ADR-SAG-0006).
//!
//! [`JevTransport`] is intentionally open. Tests and callers supply fakes.
//! [`crate::JevClient::judge`] does not take a transport.

use std::fmt;
use std::time::Duration;

use sc_agent_types::JevError;

/// One Jev HTTP POST.
///
/// The API key is the `authorization` value (`Bearer …`). It is not part of
/// `body`. [`Debug`] redacts it.
pub struct WireRequest<'a> {
    /// Absolute `https` URL.
    pub url: &'a str,
    /// Canonical check body, including the pinned model.
    pub body: &'a [u8],
    /// `Bearer` credential for this send.
    pub authorization: &'a str,
    /// Deadline for this send, including the response body.
    pub timeout: Duration,
}

impl fmt::Debug for WireRequest<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WireRequest")
            .field("url", &self.url)
            .field("body", &self.body)
            .field("authorization", &"[redacted]")
            .field("timeout", &self.timeout)
            .finish()
    }
}

/// HTTP result observed by the retry loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response body. Empty when the server sent none.
    pub body: Vec<u8>,
    /// Raw `Retry-After` header, when present.
    pub retry_after: Option<String>,
}

/// Sends one check body.
///
/// Object-safe and `Send + Sync` so a client can share it across threads.
/// Implementations must apply [`WireRequest::timeout`] and must not log
/// `authorization`.
pub trait JevTransport: Send + Sync {
    /// Posts `request` and returns the status line data.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::NetworkError`] when the connection fails. HTTP
    /// status codes, including busy replies, are [`Ok`] values.
    fn send(&self, request: &WireRequest<'_>) -> Result<WireResponse, JevError>;
}
