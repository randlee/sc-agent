//! Injectable HTTP transport for Haiku and Luna.
//!
//! Production calls use [`UreqTransport`]. Tests supply another
//! [`BackendTransport`] so judge tests do not open sockets.

use std::fmt;
use std::io::Read;
use std::time::Duration;

use sc_agent_types::{JevError, ResolvedApiKey};
use ureq::RequestExt;

/// TCP connect timeout for provider calls.
///
/// ADR-SAG-0007. Shorter values belong on the backend builder in tests.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Deadline for one provider call, including the response body.
///
/// ADR-SAG-0007 applies this to token-count and judge requests. [`UreqTransport`]
/// sets ureq `timeout_global` to this value. The connect deadline stays
/// [`DEFAULT_CONNECT_TIMEOUT`] unless the backend builder overrides it.
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
    /// This is the ADR-SAG-0020 runtime guard. The credential header is absent
    /// until this runs, and only a [`BackendTransport::send`] implementation
    /// may call it. The guard is a method rather than a typestate parameter so
    /// [`BackendTransport`] stays object-safe for `Arc<dyn BackendTransport>`.
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
/// This trait is an open extension point (ADR-SAG-0006) so tests and callers
/// supply a transport at backend construction. It is intentionally not sealed.
///
/// Implementations must honor [`HttpRequest::connect_timeout`] and
/// [`HttpRequest::request_timeout`]. They must call [`HttpRequest::install_auth`]
/// before the bytes leave the process. That call is the ADR-SAG-0020 runtime
/// guard: the credential header is absent until `install_auth` runs. The guard
/// stays a method, rather than a typestate parameter, so this trait can be
/// used as `Arc<dyn BackendTransport>`.
///
/// The backend retries busy HTTP statuses itself; the transport reports them
/// as a response rather than an error.
pub trait BackendTransport: Send + Sync {
    /// Sends `request` once.
    ///
    /// Call [`HttpRequest::install_auth`] before the request is written.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::NetworkError`] when the connection fails, times out,
    /// or the response body cannot be read. Returns
    /// [`JevError::ResponseInvalid`] when the body exceeds the transport cap.
    /// HTTP error statuses are `Ok`.
    fn send(&self, request: HttpRequest) -> Result<HttpResponse, JevError>;
}

/// Blocking provider client with per-request deadlines.
///
/// Each call refuses cleartext, does not follow redirects, and stops reading
/// a response larger than [`MAX_RESPONSE_BYTES`].
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
        if !request.url.starts_with("https://") {
            return Err(request.credential.redact_error(JevError::NetworkError {
                detail: "provider connection failed: URL is not https".to_owned(),
            }));
        }
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(request.request_timeout))
            .timeout_connect(Some(request.connect_timeout))
            .http_status_as_error(false)
            .https_only(true)
            .max_redirects(0)
            .build()
            .into();
        let mut builder = ureq::http::Request::builder()
            .method(request.method.as_str())
            .uri(request.url.as_str());
        for (name, value) in &request.headers {
            builder = builder.header(name.as_str(), value.as_str());
        }
        let outbound = match builder.body(std::mem::take(&mut request.body)) {
            Ok(outbound) => outbound,
            Err(error) => {
                return Err(request.credential.redact_error(JevError::NetworkError {
                    detail: format!("provider request is invalid: {error}"),
                }));
            }
        };
        match outbound.with_agent(&agent).run() {
            Ok(response) => {
                read_response(response).map_err(|error| request.credential.redact_error(error))
            }
            Err(error) => Err(request.credential.redact_error(JevError::NetworkError {
                detail: format!("provider connection failed: {error}"),
            })),
        }
    }
}

fn read_response(mut response: ureq::http::Response<ureq::Body>) -> Result<HttpResponse, JevError> {
    let status = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_owned(),
                String::from_utf8_lossy(value.as_bytes()).into_owned(),
            )
        })
        .collect();
    let mut reader = response
        .body_mut()
        .as_reader()
        .take(MAX_RESPONSE_BYTES.saturating_add(1));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ureq_transport_refuses_cleartext_without_sending() {
        let error = UreqTransport::new()
            .send(HttpRequest {
                method: "POST".to_owned(),
                url: "http://api.openai.com/v1/responses".to_owned(),
                headers: Vec::new(),
                body: Vec::new(),
                connect_timeout: DEFAULT_CONNECT_TIMEOUT,
                request_timeout: Duration::from_secs(1),
                credential: ResolvedApiKey::new("super-secret".to_owned()),
                scheme: AuthScheme::Bearer,
                auth_installed: false,
            })
            .expect_err("cleartext");
        assert_eq!(error.code(), "NETWORK_ERROR");
        assert!(!error.fix_hint().contains("super-secret"));
    }
}
