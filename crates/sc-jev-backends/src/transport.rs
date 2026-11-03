//! Injectable HTTP transport for Haiku and Luna.
//!
//! Production calls use [`UreqTransport`]. Tests supply another
//! [`BackendTransport`] so judge tests do not open sockets.

use std::fmt;
use std::io::Read;
use std::time::Duration;

use sc_agent_types::{JevError, ResolvedApiKey};

/// TCP connect timeout for provider calls.
///
/// ADR-SAG-0007. Shorter values belong on the backend builder in tests.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Read and write timeout for one provider call.
///
/// ADR-SAG-0007 applies this to token-count and judge requests. [`UreqTransport`]
/// sets both the read and the write deadline to this value. The connect deadline
/// stays [`DEFAULT_CONNECT_TIMEOUT`] unless the backend builder overrides it.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// Largest provider response body this crate will buffer.
///
/// A 16_000-token JSON answer plus usage metadata fits well under this.
/// Anything larger is refused so a response cannot grow without a bound.
const MAX_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;

/// How the sending transport attaches [`ResolvedApiKey`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuthScheme {
    /// Anthropic `x-api-key` header.
    AnthropicApiKey,
    /// `Authorization: Bearer` header.
    Bearer,
}

/// One outbound provider call.
///
/// `credential` stays sealed until [`Self::install_auth`], which a
/// [`BackendTransport::send`] implementation calls before the bytes leave
/// the process (ADR-SAG-0020).
#[derive(Clone, PartialEq, Eq)]
pub struct HttpRequest {
    /// HTTP method. Provider calls use `POST`.
    pub method: String,
    /// Absolute URL, including host and path.
    pub url: String,
    /// Request headers in send order. Names keep the caller's casing.
    ///
    /// The credential header is absent until [`Self::install_auth`].
    pub headers: Vec<(String, String)>,
    /// Raw JSON body.
    pub body: Vec<u8>,
    /// TCP connect deadline applied by the transport.
    pub connect_timeout: Duration,
    /// Read and write deadline applied by the transport.
    pub request_timeout: Duration,
    pub(crate) credential: ResolvedApiKey,
    pub(crate) scheme: AuthScheme,
    pub(crate) auth_installed: bool,
}

impl HttpRequest {
    /// Copies the sealed credential into the provider auth header.
    ///
    /// Only a [`BackendTransport::send`] implementation may call this.
    pub fn install_auth(&mut self) {
        if self.auth_installed {
            return;
        }
        let header = match self.scheme {
            AuthScheme::AnthropicApiKey => {
                ("x-api-key".to_owned(), self.credential.as_str().to_owned())
            }
            AuthScheme::Bearer => (
                "Authorization".to_owned(),
                format!("Bearer {}", self.credential.as_str()),
            ),
        };
        self.headers.insert(0, header);
        self.auth_installed = true;
    }
}

impl fmt::Debug for HttpRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpRequest")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("headers", &"[redacted]")
            .field("body", &self.body)
            .field("connect_timeout", &self.connect_timeout)
            .field("request_timeout", &self.request_timeout)
            .field("credential", &self.credential)
            .finish()
    }
}

/// One provider response, including non-200 statuses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response headers. Names keep the transport's casing.
    pub headers: Vec<(String, String)>,
    /// Raw response body, capped by the transport.
    pub body: Vec<u8>,
}

/// Sends one HTTP request.
///
/// Implementations must honor [`HttpRequest::connect_timeout`] and
/// [`HttpRequest::request_timeout`]. The backend retries busy HTTP statuses
/// itself; the transport reports them as a response rather than an error.
pub trait BackendTransport: Send + Sync {
    /// Sends `request` once.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::NetworkError`] when the connection fails, times out,
    /// or the response body cannot be read. HTTP error statuses are `Ok`.
    fn send(&self, request: HttpRequest) -> Result<HttpResponse, JevError>;
}

/// Blocking provider client with per-request deadlines.
#[derive(Debug, Default, Clone, Copy)]
pub struct UreqTransport;

impl UreqTransport {
    /// Returns a transport that opens a fresh agent for each call.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl BackendTransport for UreqTransport {
    fn send(&self, mut request: HttpRequest) -> Result<HttpResponse, JevError> {
        request.install_auth();
        let agent = ureq::AgentBuilder::new()
            .timeout(request.request_timeout)
            .timeout_connect(request.connect_timeout)
            .build();
        let mut outbound = agent.request(&request.method, &request.url);
        for (name, value) in &request.headers {
            outbound = outbound.set(name, value);
        }
        match outbound.send_bytes(&request.body) {
            Ok(response) => {
                read_response(response).map_err(|error| request.credential.redact_error(error))
            }
            Err(ureq::Error::Status(_, response)) => {
                read_response(response).map_err(|error| request.credential.redact_error(error))
            }
            Err(ureq::Error::Transport(error)) => {
                Err(request.credential.redact_error(JevError::NetworkError {
                    detail: format!("provider connection failed: {error}"),
                }))
            }
        }
    }
}

fn read_response(response: ureq::Response) -> Result<HttpResponse, JevError> {
    let status = response.status();
    let headers = response
        .headers_names()
        .into_iter()
        .filter_map(|name| response.header(&name).map(|value| (name, value.to_owned())))
        .collect();
    let mut reader = response.into_reader().take(MAX_RESPONSE_BYTES + 1);
    let mut body = Vec::new();
    reader
        .read_to_end(&mut body)
        .map_err(|error| JevError::NetworkError {
            detail: format!("reading provider response failed: {error}"),
        })?;
    if body.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(JevError::ResponseInvalid {
            detail: format!("provider response exceeds {MAX_RESPONSE_BYTES} bytes"),
        });
    }
    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}
