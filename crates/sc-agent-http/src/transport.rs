//! The [`Transport`] trait and transport [`Timeouts`].

use std::time::Duration;

use sc_agent_types::JevError;

use crate::request::{HttpRequest, HttpResponse};

/// Sends one HTTP request, once.
///
/// Open on purpose (ADR-SAG-0006): backends take a transport at
/// construction, and tests supply their own. Object-safe and `Send + Sync`.
///
/// Calling `send` directly bypasses busy retry **and** redaction. Production
/// code calls only [`send_with_busy_retry`](crate::send_with_busy_retry),
/// which adds both. A transport other than
/// [`UreqTransport`](crate::UreqTransport) cannot read the key in
/// [`HttpRequest::secret`]; it sees only the header name,
/// [`SecretHeader::redacted`](crate::SecretHeader::redacted) and
/// [`SecretHeader::matches`](crate::SecretHeader::matches).
pub trait Transport: Send + Sync {
    /// Sends `request` and returns the response, whatever its status.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::NetworkError`] when the connection, the send or
    /// the body read fails or times out, and [`JevError::ResponseInvalid`]
    /// when the body exceeds the transport cap. HTTP statuses, including busy
    /// and error statuses, are `Ok`.
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, JevError>;
}

impl<T: Transport + ?Sized> Transport for &T {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, JevError> {
        (**self).send(request)
    }
}

impl<T: Transport + ?Sized> Transport for std::sync::Arc<T> {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, JevError> {
        (**self).send(request)
    }
}

/// Connect and whole-request deadlines of a transport (ADR-SAG-0007).
///
/// The request deadline covers the send and the full body read. The defaults
/// are 10 s connect and 120 s request.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
///
/// use sc_agent_http::Timeouts;
///
/// let timeouts = Timeouts::default();
/// assert_eq!(timeouts.connect(), Duration::from_secs(10));
/// assert_eq!(timeouts.request(), Duration::from_secs(120));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Timeouts {
    connect: Duration,
    request: Duration,
}

impl Timeouts {
    /// Default TCP connect deadline.
    pub const DEFAULT_CONNECT: Duration = Duration::from_secs(10);

    /// Default deadline for one request, including the response body.
    pub const DEFAULT_REQUEST: Duration = Duration::from_secs(120);

    /// Timeouts with the given connect and request deadlines.
    #[must_use]
    pub const fn new(connect: Duration, request: Duration) -> Self {
        Self { connect, request }
    }

    /// TCP connect deadline.
    #[must_use]
    pub const fn connect(self) -> Duration {
        self.connect
    }

    /// Deadline for one request, including the response body.
    #[must_use]
    pub const fn request(self) -> Duration {
        self.request
    }
}

impl Default for Timeouts {
    fn default() -> Self {
        Self::new(Self::DEFAULT_CONNECT, Self::DEFAULT_REQUEST)
    }
}
