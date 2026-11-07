//! Production transport over ureq (private module; [`UreqTransport`] is
//! re-exported at the crate root).
//!
//! This file is the workspace's single `ResolvedApiKey::as_str` call site
//! (ADR-SAG-0020 r-3 amendment): [`expose`] carries the one
//! `#[expect(clippy::disallowed_methods)]`. URL parsing uses ureq's
//! re-exported `http::Uri`; no ureq type leaves this module.

use std::io::Read;
use std::str::FromStr;
use std::time::Duration;

use sc_agent_types::{JevError, NetworkErrorKind, ResolvedApiKey};
use ureq::http::Uri;

use crate::request::{HttpRequest, HttpResponse, SecretHeader};
use crate::transport::{Timeouts, Transport};

/// Largest response body read, error statuses included (8 MiB).
const MAX_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;

/// `User-Agent` sent on every request.
const USER_AGENT: &str = concat!("sc-agent-http/", env!("CARGO_PKG_VERSION"));

/// Every setting of the one ureq agent a [`UreqTransport`] builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AgentConfig {
    connect: Duration,
    global: Duration,
    max_redirects: u32,
    https_only: bool,
    user_agent: &'static str,
}

impl AgentConfig {
    /// HTTPS only, no redirects, versioned `User-Agent`, `timeouts`.
    fn production(timeouts: Timeouts) -> Self {
        Self {
            connect: timeouts.connect(),
            global: timeouts.request(),
            max_redirects: 0,
            https_only: true,
            user_agent: USER_AGENT,
        }
    }

    fn build(self) -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_connect(Some(self.connect))
            .timeout_global(Some(self.global))
            .max_redirects(self.max_redirects)
            .https_only(self.https_only)
            .user_agent(self.user_agent)
            .http_status_as_error(false)
            .build()
            .into()
    }
}

/// Blocking HTTPS transport.
///
/// [`UreqTransport::new`] builds **one** agent and every send reuses it, so
/// retries and later calls share its connection pool. The agent connects
/// within the connect timeout (10 s by default), bounds each whole request,
/// body read included, by the request timeout (120 s by default), follows no
/// redirect, refuses plain HTTP and sends `User-Agent: sc-agent-http/<ver>`.
/// A response body over 8 MiB is refused while streaming, for error statuses
/// too. This is the only transport that writes a [`SecretHeader`] value.
///
/// # Examples
///
/// ```
/// use sc_agent_http::{Timeouts, UreqTransport};
///
/// let transport = UreqTransport::new(Timeouts::default());
/// let _shared = transport.clone(); // clones share the agent
/// ```
#[derive(Debug, Clone)]
pub struct UreqTransport {
    agent: ureq::Agent,
    config: AgentConfig,
}

impl UreqTransport {
    /// A transport with one HTTPS-only agent using `timeouts`.
    #[must_use]
    pub fn new(timeouts: Timeouts) -> Self {
        Self::from_config(AgentConfig::production(timeouts))
    }

    /// The production agent settings with HTTPS-only off, for
    /// `Endpoint::loopback_http` targets.
    ///
    /// Only an `Endpoint::loopback_http` endpoint is plain HTTP, and it can
    /// only name `127.0.0.1`.
    #[cfg(feature = "test-util")]
    #[must_use]
    pub fn loopback_for_tests(timeouts: Timeouts) -> Self {
        Self::from_config(AgentConfig {
            https_only: false,
            ..AgentConfig::production(timeouts)
        })
    }

    fn from_config(config: AgentConfig) -> Self {
        Self {
            agent: config.build(),
            config,
        }
    }
}

impl Default for UreqTransport {
    fn default() -> Self {
        Self::new(Timeouts::default())
    }
}

impl Transport for UreqTransport {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, JevError> {
        if self.config.https_only && !request.endpoint().is_https() {
            return Err(JevError::network(
                NetworkErrorKind::Tls,
                "connection refused: endpoint is not https",
            ));
        }
        let mut builder = ureq::http::Request::builder()
            .method(request.method().as_str())
            .uri(request.endpoint().as_str());
        for (name, value) in request.headers() {
            builder = builder.header(name.as_str(), value.as_str());
        }
        if let Some(secret) = request.secret() {
            let value = format!("{}{}", secret.prefix(), expose(secret.key()));
            builder = builder.header(secret.name().as_str(), value);
        }
        let outbound = builder.body(request.body()).map_err(|error| {
            JevError::network(
                NetworkErrorKind::Connect,
                format!("request is invalid: {error}"),
            )
        })?;
        let response = self.agent.run(outbound).map_err(|error| {
            JevError::network(
                ureq_error_kind(&error),
                format!("connection failed: {error}"),
            )
        })?;
        read_response(response)
    }
}

impl SecretHeader {
    /// Whether the key equals `candidate` (without the scheme prefix).
    ///
    /// For test transports, which cannot read the key.
    #[must_use]
    pub fn matches(&self, candidate: &str) -> bool {
        expose(self.key()) == candidate
    }
}

/// The single secret read site of the workspace (ADR-SAG-0020).
#[expect(
    clippy::disallowed_methods,
    reason = "single secret read site (ADR-SAG-0020)"
)]
fn expose(key: &ResolvedApiKey) -> &str {
    key.as_str()
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
    reader.read_to_end(&mut body).map_err(|error| {
        JevError::network(
            io_error_kind(&error),
            format!("reading the response failed: {error}"),
        )
    })?;
    if body.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(JevError::response_invalid(format!(
            "HTTP {status} response exceeds {MAX_RESPONSE_BYTES} bytes"
        )));
    }
    Ok(HttpResponse::new(status, headers, body))
}

/// `Ok` when `url` is an absolute `https` URL with a host.
pub(crate) fn check_https_url(url: &str) -> Result<(), JevError> {
    let uri = Uri::from_str(url).map_err(|error| {
        JevError::config_invalid(format!("endpoint {url:?} is invalid: {error}"))
    })?;
    if uri.scheme_str() != Some("https") {
        return Err(JevError::config_invalid(format!(
            "endpoint {url:?} must use https"
        )));
    }
    if uri.host().is_none_or(str::is_empty) {
        return Err(JevError::config_invalid(format!(
            "endpoint {url:?} has no host"
        )));
    }
    Ok(())
}

/// Whether `url` parses as an absolute URL.
#[cfg(feature = "test-util")]
pub(crate) fn is_valid_url(url: &str) -> bool {
    Uri::from_str(url).is_ok_and(|uri| uri.scheme().is_some() && uri.host().is_some())
}

/// Classifies a ureq failure for [`JevError::retryable`].
///
/// TLS, certificate and https-only refusals are `Tls` (not retryable); an
/// oversize body is `BodyTooLarge`; a timeout is `Timeout`; everything else
/// that fails before a response, DNS included, is `Connect`.
fn ureq_error_kind(error: &ureq::Error) -> NetworkErrorKind {
    match error {
        ureq::Error::Timeout(_) => NetworkErrorKind::Timeout,
        ureq::Error::Io(io) => io_error_kind(io),
        ureq::Error::Tls(_)
        | ureq::Error::TlsRequired
        | ureq::Error::RequireHttpsOnly(_)
        | ureq::Error::Pem(_)
        | ureq::Error::Rustls(_) => NetworkErrorKind::Tls,
        ureq::Error::BodyExceedsLimit(_) => NetworkErrorKind::BodyTooLarge,
        _ => NetworkErrorKind::Connect,
    }
}

/// `Timeout` for a timed-out read, otherwise `Connect`.
fn io_error_kind(error: &std::io::Error) -> NetworkErrorKind {
    if error.kind() == std::io::ErrorKind::TimedOut {
        NetworkErrorKind::Timeout
    } else {
        NetworkErrorKind::Connect
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Endpoint;

    #[test]
    fn test_endpoint_parse_refuses_http() {
        for url in [
            "http://api.openai.com/v1/responses",
            "http://127.0.0.1:8080/x",
            "HTTP://api.openai.com/",
            "ftp://api.openai.com/",
            "api.openai.com/v1",
            "https://",
            "",
        ] {
            let error = Endpoint::parse(url).expect_err(url);
            assert_eq!(error.code(), "CONFIG_INVALID", "{url}");
        }
        let endpoint = Endpoint::parse("https://api.openai.com/v1/responses").expect("https");
        assert!(endpoint.is_https());
    }

    #[test]
    fn test_ureq_agent_config_values() {
        let config = UreqTransport::new(Timeouts::default()).config;
        assert_eq!(
            config,
            AgentConfig {
                connect: Duration::from_secs(10),
                global: Duration::from_secs(120),
                max_redirects: 0,
                https_only: true,
                user_agent: USER_AGENT,
            }
        );
        assert_eq!(
            USER_AGENT,
            format!("sc-agent-http/{}", env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(UreqTransport::default().config, config);
        let custom = UreqTransport::new(Timeouts::new(
            Duration::from_secs(1),
            Duration::from_secs(60),
        ))
        .config;
        assert_eq!(
            (custom.connect, custom.global),
            (Duration::from_secs(1), Duration::from_secs(60))
        );
        assert!(custom.https_only);
    }

    #[test]
    fn ureq_errors_map_to_network_kinds() {
        let cases = [
            (
                ureq::Error::Timeout(ureq::Timeout::Global),
                NetworkErrorKind::Timeout,
            ),
            (
                ureq::Error::Io(std::io::Error::from(std::io::ErrorKind::TimedOut)),
                NetworkErrorKind::Timeout,
            ),
            (
                ureq::Error::Io(std::io::Error::from(std::io::ErrorKind::ConnectionReset)),
                NetworkErrorKind::Connect,
            ),
            (ureq::Error::Tls("handshake"), NetworkErrorKind::Tls),
            (ureq::Error::TlsRequired, NetworkErrorKind::Tls),
            (
                ureq::Error::BodyExceedsLimit(1),
                NetworkErrorKind::BodyTooLarge,
            ),
            (ureq::Error::ConnectionFailed, NetworkErrorKind::Connect),
            (ureq::Error::HostNotFound, NetworkErrorKind::Connect),
        ];
        for (error, kind) in cases {
            assert_eq!(ureq_error_kind(&error), kind, "{error}");
        }
        assert!(NetworkErrorKind::Connect.retryable());
        assert!(!NetworkErrorKind::Tls.retryable());
    }
}
