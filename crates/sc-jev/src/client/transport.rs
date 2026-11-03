//! Injectable HTTP transport (ADR-SAG-0006).
//!
//! [`JevTransport`] is intentionally open. Tests and callers supply fakes.
//! [`crate::JevClient::judge`] does not take a transport.

use std::fmt;
use std::time::Duration;

use sc_agent_types::{JevError, ResolvedApiKey};

/// One Jev HTTP POST.
///
/// `api_key` stays sealed on the request. [`crate::JevTransport::send`] is
/// the only caller that may read it, via [`ResolvedApiKey::as_str`], when it
/// sets the `Authorization` header (ADR-SAG-0020). [`Debug`] redacts the key.
pub struct WireRequest<'a> {
    /// Absolute `https` URL.
    pub url: &'a str,
    /// Canonical check body, including the pinned model.
    pub body: &'a [u8],
    /// Sealed TypeSafe credential. The transport formats `Bearer` at send time.
    pub api_key: &'a ResolvedApiKey,
    /// Deadline for this send, including the response body.
    pub timeout: Duration,
}

impl fmt::Debug for WireRequest<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WireRequest")
            .field("url", &self.url)
            .field("body", &self.body)
            .field("api_key", &self.api_key)
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
/// Implementations must apply [`WireRequest::timeout`], may call
/// [`ResolvedApiKey::as_str`] only while sending, and must not log the key.
pub trait JevTransport: Send + Sync {
    /// Posts `request` and returns the status line data.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::NetworkError`] when the connection fails. HTTP
    /// status codes, including busy replies, are [`Ok`] values.
    fn send(&self, request: &WireRequest<'_>) -> Result<WireResponse, JevError>;
}
