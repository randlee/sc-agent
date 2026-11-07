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
/// ADR-SAG-0007. Tests shorten it with the backends' `with_timeouts` builder.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Deadline for one provider call, including the response body.
///
/// ADR-SAG-0007 applies this to token-count and judge requests. [`UreqTransport`]
/// sets ureq `timeout_global` to this value. The connect deadline stays
/// [`DEFAULT_CONNECT_TIMEOUT`] unless the backends' `with_timeouts` builder
/// overrides it.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// Largest provider response body this crate will buffer.
///
/// A 16_000-token JSON answer plus usage metadata fits well under this.
/// Anything larger is refused so a response cannot grow without a bound.
const MAX_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;

/// `User-Agent` sent on every provider call.
const USER_AGENT: &str = concat!("sc-jev-backends/", env!("CARGO_PKG_VERSION"));

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
        #[allow(
            clippy::disallowed_methods,
            reason = "ADR-SAG-0020: install_auth runs inside the sending transport only"
        )]
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
/// One ureq agent is built in [`Self::new`] and reused for every call, so
/// the count call, the judge call, and busy retries share its connection
/// pool. The agent refuses cleartext, does not follow redirects, and sends a
/// versioned `User-Agent`. Each call applies the request's connect and
/// request timeouts and stops reading a response larger than 8 MiB.
#[derive(Debug, Clone)]
pub struct UreqTransport {
    agent: ureq::Agent,
    require_https: bool,
}

impl UreqTransport {
    /// Returns a transport with one reusable HTTPS-only agent.
    #[must_use]
    pub fn new() -> Self {
        Self {
            agent: build_agent(true),
            require_https: true,
        }
    }

    /// Loopback-only variant for transport tests; allows `http://`.
    #[cfg(test)]
    pub(crate) fn loopback_for_tests() -> Self {
        Self {
            agent: build_agent(false),
            require_https: false,
        }
    }
}

impl Default for UreqTransport {
    fn default() -> Self {
        Self::new()
    }
}

fn build_agent(https_only: bool) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(DEFAULT_CONNECT_TIMEOUT))
        .http_status_as_error(false)
        .https_only(https_only)
        .max_redirects(0)
        .user_agent(USER_AGENT)
        .build()
        .into()
}

impl BackendTransport for UreqTransport {
    fn send(&self, mut request: HttpRequest) -> Result<HttpResponse, JevError> {
        request.install_auth();
        if self.require_https && !request.url.starts_with("https://") {
            return Err(request.credential.redact_error(JevError::NetworkError {
                detail: "provider connection failed: URL is not https".to_owned(),
            }));
        }
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
        let result = outbound
            .with_agent(&self.agent)
            .configure()
            .timeout_connect(Some(request.connect_timeout))
            .timeout_global(Some(request.request_timeout))
            .build()
            .run();
        match result {
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
    use crate::test_server::{LoopbackServer, Reply, closed_port_url};

    const KEY: &str = "sk-loopback-secret";

    fn request(url: &str, scheme: AuthScheme, request_timeout: Duration) -> HttpRequest {
        HttpRequest {
            method: "POST".to_owned(),
            url: url.to_owned(),
            headers: vec![("Content-Type".to_owned(), "application/json".to_owned())],
            body: br#"{"q":1}"#.to_vec(),
            connect_timeout: Duration::from_secs(2),
            request_timeout,
            credential: ResolvedApiKey::new(KEY.to_owned()),
            scheme,
            auth_installed: false,
        }
    }

    fn assert_no_key(error: &JevError) {
        let rendered = format!("{error} {error:?} {}", error.fix_hint());
        assert!(!rendered.contains(KEY), "{rendered}");
    }

    #[test]
    fn ureq_transport_refuses_cleartext_without_sending() {
        let error = UreqTransport::new()
            .send(request(
                "http://api.openai.com/v1/responses",
                AuthScheme::Bearer,
                Duration::from_secs(1),
            ))
            .expect_err("cleartext");
        assert_eq!(error.code(), "NETWORK_ERROR");
        assert_no_key(&error);
    }

    #[test]
    fn install_auth_is_idempotent_and_debug_redacts() {
        let mut outbound = request(
            "https://x.invalid/",
            AuthScheme::AnthropicApiKey,
            Duration::ZERO,
        );
        outbound.install_auth();
        outbound.install_auth();
        assert_eq!(outbound.headers.len(), 2);
        assert_eq!(
            outbound.headers[0],
            ("x-api-key".to_owned(), KEY.to_owned())
        );
        assert!(!format!("{outbound:?}").contains(KEY));
    }

    #[test]
    fn sends_auth_headers_body_and_reads_reply_headers() {
        let server = LoopbackServer::start(vec![
            Reply::status(200, &[("Retry-After", "2")], b"{\"ok\":1}"),
            Reply::status(200, &[], b"{}"),
        ]);
        let transport = UreqTransport::loopback_for_tests();
        let response = transport
            .send(request(
                &server.url("/v1/messages"),
                AuthScheme::AnthropicApiKey,
                Duration::from_secs(5),
            ))
            .expect("send");
        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"{\"ok\":1}");
        assert!(
            response
                .headers
                .iter()
                .any(|(name, value)| name.eq_ignore_ascii_case("retry-after") && value == "2")
        );
        transport
            .send(request(
                &server.url("/v1/responses"),
                AuthScheme::Bearer,
                Duration::from_secs(5),
            ))
            .expect("second send reuses the agent");
        let seen = server.finish();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].header("x-api-key"), Some(KEY));
        assert_eq!(seen[0].header("authorization"), None);
        assert_eq!(seen[0].header("user-agent"), Some(USER_AGENT));
        assert!(USER_AGENT.starts_with("sc-jev-backends/"));
        assert_eq!(seen[0].body, br#"{"q":1}"#);
        assert_eq!(
            seen[1].header("authorization"),
            Some(format!("Bearer {KEY}").as_str())
        );
    }

    #[test]
    fn error_statuses_and_redirects_are_responses() {
        let target = closed_port_url();
        let server = LoopbackServer::start(vec![
            Reply::status(429, &[("Retry-After", "1.5")], b"busy"),
            Reply::status(500, &[], b"boom"),
            Reply::status(301, &[("Location", target.as_str())], b""),
        ]);
        let transport = UreqTransport::loopback_for_tests();
        let mut statuses = Vec::new();
        for path in ["/a", "/b", "/c"] {
            let response = transport
                .send(request(
                    &server.url(path),
                    AuthScheme::Bearer,
                    Duration::from_secs(5),
                ))
                .expect("status is a response");
            statuses.push(response.status);
        }
        assert_eq!(statuses, [429, 500, 301]);
        assert_eq!(server.finish().len(), 3);
    }

    #[test]
    fn oversize_body_is_refused() {
        let body = vec![b'x'; usize::try_from(MAX_RESPONSE_BYTES).unwrap() + 1];
        let server = LoopbackServer::start(vec![Reply::status(200, &[], &body)]);
        let error = UreqTransport::loopback_for_tests()
            .send(request(
                &server.url("/big"),
                AuthScheme::Bearer,
                Duration::from_secs(30),
            ))
            .expect_err("oversize");
        assert_eq!(error.code(), "RESPONSE_INVALID");
        assert!(error.to_string().contains("exceeds"));
        server.finish();
    }

    #[test]
    fn timeout_refused_and_malformed_requests_are_network_errors() {
        let server = LoopbackServer::start(vec![Reply::Stall(Duration::from_millis(800))]);
        let transport = UreqTransport::loopback_for_tests();
        let error = transport
            .send(request(
                &server.url("/slow"),
                AuthScheme::Bearer,
                Duration::from_millis(150),
            ))
            .expect_err("timeout");
        assert_eq!(error.code(), "NETWORK_ERROR");
        assert!(error.to_string().contains("provider connection failed"));
        assert_no_key(&error);
        server.finish();

        let refused = transport
            .send(request(
                &closed_port_url(),
                AuthScheme::Bearer,
                Duration::from_secs(2),
            ))
            .expect_err("refused");
        assert_eq!(refused.code(), "NETWORK_ERROR");
        assert_no_key(&refused);

        let mut bad = request(
            "http://127.0.0.1:9/x",
            AuthScheme::Bearer,
            Duration::from_secs(1),
        );
        bad.headers.push(("bad header".to_owned(), "v".to_owned()));
        let invalid = transport.send(bad).expect_err("invalid header name");
        assert_eq!(invalid.code(), "NETWORK_ERROR");
        assert!(invalid.to_string().contains("provider request is invalid"));
        assert_no_key(&invalid);
    }
}
