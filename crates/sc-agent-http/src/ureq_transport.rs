//! Production transport over ureq (private module; [`UreqTransport`] is
//! re-exported at the crate root).
//!
//! This file is the workspace's single `ApiKey::as_str` call site: [`expose`]
//! carries the one `#[expect(clippy::disallowed_methods)]`. No ureq type
//! leaves this module.

use std::io::Read;
use std::time::Duration;

use sc_agent_secrets::ApiKey;
use sc_agent_types::ErrorDetail;
use ureq::http::Uri;
use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::{DefaultConnector, NextTimeout};

use crate::error::{HttpError, NetworkKind};
use crate::request::{HttpRequest, HttpResponse, SecretHeader};
use crate::transport::{Timeouts, Transport};

/// Largest response body read, error statuses included (8 MiB).
const MAX_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;

/// `User-Agent` sent on every request.
const USER_AGENT: &str = concat!("sc-agent-http/", env!("CARGO_PKG_VERSION"));

/// Idle connections kept per host, and in total. A runner sends many
/// requests at once to one host; ureq's default of 3 idle per host would
/// close and reopen most of them after every burst.
const MAX_IDLE_CONNECTIONS: usize = 64;

/// Every setting of the one ureq agent a [`UreqTransport`] builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AgentConfig {
    connect: Duration,
    global: Duration,
    max_redirects: u32,
    user_agent: &'static str,
}

impl AgentConfig {
    /// No redirects, versioned `User-Agent`, `timeouts`.
    fn production(timeouts: Timeouts) -> Self {
        Self {
            connect: timeouts.connect,
            global: timeouts.request,
            max_redirects: 0,
            user_agent: USER_AGENT,
        }
    }

    /// The agent: `timeout_connect` is the connect deadline and
    /// `timeout_global` one deadline over connect, headers and the whole body
    /// read. Every status is a response (`http_status_as_error(false)`).
    fn build(self) -> ureq::Agent {
        let config = ureq::Agent::config_builder()
            .max_idle_connections(MAX_IDLE_CONNECTIONS)
            .max_idle_connections_per_host(MAX_IDLE_CONNECTIONS)
            .timeout_connect(Some(self.connect))
            .timeout_global(Some(self.global))
            .max_redirects(self.max_redirects)
            .user_agent(self.user_agent)
            .http_status_as_error(false)
            .build();
        ureq::Agent::with_parts(
            config,
            DefaultConnector::default(),
            LookupResolver::default(),
        )
    }
}

/// ureq's own resolver, with a failed lookup reported as `HostNotFound`.
///
/// `getaddrinfo` failures reach ureq as `Error::Io` with an uncategorized
/// `io::ErrorKind` (std has no kind for them), so they cannot be told apart
/// from other I/O errors by kind. Only the name lookup runs here, so any I/O
/// error from it is a lookup failure and becomes `HostNotFound` (`connect`)
/// without reading any message text. A lookup timeout and every other ureq
/// error pass through.
#[derive(Debug, Default)]
struct LookupResolver {
    inner: DefaultResolver,
}

impl Resolver for LookupResolver {
    fn resolve(
        &self,
        uri: &Uri,
        config: &ureq::config::Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        self.inner
            .resolve(uri, config, timeout)
            .map_err(|error| match error {
                ureq::Error::Io(_) => ureq::Error::HostNotFound,
                other => other,
            })
    }
}

/// Blocking HTTP transport over one ureq agent.
///
/// [`UreqTransport::new`] builds **one** agent and every send reuses it, so
/// resends and later calls share its connection pool. The agent connects
/// within [`Timeouts::connect`], bounds each whole send, body read included,
/// by [`Timeouts::request`] (or the request's own
/// [`HttpRequest::deadline`]), follows no redirect and sends
/// `User-Agent: sc-agent-http/<ver>`. Plain http is reachable only through
/// [`Endpoint::parse_local`](crate::Endpoint::parse_local). A response body
/// over 8 MiB is refused while streaming, for error statuses too. This is the
/// only transport that writes a [`SecretHeader`] value.
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
    timeouts: Timeouts,
}

impl UreqTransport {
    /// A transport with one agent using `timeouts`.
    ///
    /// `timeouts` are checked at each send: a zero value or one over
    /// [`Timeouts::MAX`] makes every send [`HttpError::InvalidRequest`] (a
    /// request's own deadline may go up to [`Timeouts::DEADLINE_MAX`]),
    /// without a connection.
    #[must_use]
    pub fn new(timeouts: Timeouts) -> Self {
        Self {
            agent: AgentConfig::production(timeouts).build(),
            timeouts,
        }
    }
}

impl Default for UreqTransport {
    /// [`Timeouts::default`]: connect 10 s, request 120 s.
    fn default() -> Self {
        Self::new(Timeouts::default())
    }
}

impl Transport for UreqTransport {
    /// The timeouts are in bounds ([`Timeouts::validate`]).
    fn ready(&self) -> Result<(), HttpError> {
        self.timeouts.validate().map(drop)
    }

    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError> {
        self.timeouts.validate()?;
        if let Some(deadline) = request.deadline() {
            crate::transport::check_duration(
                "the request deadline",
                deadline,
                Timeouts::DEADLINE_MAX,
            )?;
        }
        let mut outbound = build_outbound(request)?;
        if let Some(deadline) = request.deadline() {
            // Replaces the agent's request deadline for this send only.
            outbound = self
                .agent
                .configure_request(outbound)
                .timeout_global(Some(deadline))
                .build();
        }
        let response = self.agent.run(outbound).map_err(ureq_error)?;
        read_response(response)
    }
}

/// The outbound ureq request for `request`, credential header included.
fn build_outbound(request: &HttpRequest) -> Result<ureq::http::Request<&[u8]>, HttpError> {
    let mut builder = ureq::http::Request::builder()
        .method(request.method().as_str())
        .uri(request.endpoint().as_str());
    for (name, value) in request.headers() {
        builder = builder.header(name.as_str(), value.as_str());
    }
    if let Some(secret) = request.secret() {
        let value = secret_header_value(secret)?;
        builder = builder.header(secret.name(), value);
    }
    // A request that cannot be built (bad header name or value) is
    // deterministic, so it is `http.invalid_request`, never resent. The http
    // builder's error names the problem, never the value.
    builder.body(request.body()).map_err(|error| {
        HttpError::invalid_request("the HTTP builder refused a header name or value")
            .with_cause(ErrorDetail::foreign("http.builder", &error))
    })
}

/// The credential header value, marked sensitive so `Debug` of the built
/// request and HTTP/2 header compression never expose it.
///
/// `ApiKey` already refuses whitespace, control characters and anything
/// outside visible ASCII, which is stricter than `http`'s header-value check,
/// so the error is defence in depth; its text never contains the key.
fn secret_header_value(secret: &SecretHeader) -> Result<ureq::http::HeaderValue, HttpError> {
    let text = format!("{}{}", secret.prefix(), expose(secret.key()));
    let mut value = ureq::http::HeaderValue::from_str(&text).map_err(|_| {
        HttpError::invalid_request(format!("the {} header value is not valid", secret.name()))
    })?;
    value.set_sensitive(true);
    Ok(value)
}

/// The single secret read site of the workspace.
#[expect(
    clippy::disallowed_methods,
    reason = "the single ApiKey::as_str call site: the transport that sends the key (REQ-SAG-0010, A-5)"
)]
fn expose(key: &ApiKey) -> &str {
    key.as_str()
}

fn read_response(
    mut response: ureq::http::Response<ureq::Body>,
) -> Result<HttpResponse, HttpError> {
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
        .map_err(|error| body_read_error(status, &error))?;
    if body.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(HttpError::ResponseTooLarge {
            status,
            limit_bytes: MAX_RESPONSE_BYTES,
        });
    }
    Ok(HttpResponse::new(status, headers, body))
}

/// A failed body read: ureq reports its own errors (the global deadline
/// included) through `io::Error`, with the `ureq::Error` inside.
fn body_read_error(status: u16, error: &std::io::Error) -> HttpError {
    match error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<ureq::Error>())
    {
        Some(ureq::Error::BodyExceedsLimit(_)) => HttpError::ResponseTooLarge {
            status,
            limit_bytes: MAX_RESPONSE_BYTES,
        },
        Some(inner) => network(ureq_error_kind(inner), &foreign_id(inner), inner, true),
        None => network(io_error_kind(error), &io_id(error), error, true),
    }
}

/// A ureq failure as an [`HttpError`].
///
/// An oversize body is `http.response_too_large` (status 0: ureq refused it
/// before a status was read), the same rule as the streaming cap in
/// [`read_response`]; everything else is `http.network` of
/// [`ureq_error_kind`], with the ureq error kept as the cause.
fn ureq_error(error: ureq::Error) -> HttpError {
    if let ureq::Error::BodyExceedsLimit(_) = error {
        return HttpError::ResponseTooLarge {
            status: 0,
            limit_bytes: MAX_RESPONSE_BYTES,
        };
    }
    network(ureq_error_kind(&error), &foreign_id(&error), &error, false)
}

/// `http.network` in this layer's words, with `error` unchanged as the cause
/// ([`ErrorDetail::foreign`] under `id`). `attempts` is 1 (this send); the
/// client sets the real count.
fn network(
    kind: NetworkKind,
    id: &str,
    error: &(dyn std::error::Error + 'static),
    reading_body: bool,
) -> HttpError {
    let what = match (reading_body, kind) {
        (true, _) => "reading the response failed",
        (false, NetworkKind::Timeout) => "the request timed out",
        (false, NetworkKind::Connect) => "could not connect",
        (false, NetworkKind::Tls) => "the TLS handshake failed",
        (false, _) => "the transport failed",
    };
    HttpError::network(kind, 1, what).with_cause(ErrorDetail::foreign(id, error))
}

/// `io.<ErrorKind>`, the id of a foreign io error.
fn io_id(error: &std::io::Error) -> String {
    format!("io.{:?}", error.kind())
}

/// The id of a ureq error: `io.<ErrorKind>` for an I/O error,
/// `ureq.Timeout.<which>` for a timeout, else `ureq.<Variant>`.
fn foreign_id(error: &ureq::Error) -> String {
    match error {
        ureq::Error::Io(io) => io_id(io),
        ureq::Error::Timeout(which) => format!("ureq.Timeout.{which:?}"),
        other => {
            let debug = format!("{other:?}");
            let variant: String = debug
                .chars()
                .take_while(char::is_ascii_alphanumeric)
                .collect();
            if variant.is_empty() {
                "ureq.Error".to_owned()
            } else {
                format!("ureq.{variant}")
            }
        }
    }
}

/// Classifies a ureq failure.
///
/// TLS and certificate failures are `Tls`; a timeout is `Timeout`; a
/// connection-class I/O error, a DNS failure (made `HostNotFound` by
/// [`LookupResolver`]) or a proxy-connect failure is `Connect`; any other
/// error is `Other`.
fn ureq_error_kind(error: &ureq::Error) -> NetworkKind {
    match error {
        ureq::Error::Timeout(_) | ureq::Error::BodyStalled => NetworkKind::Timeout,
        ureq::Error::Io(io) => io_error_kind(io),
        ureq::Error::Tls(_)
        | ureq::Error::TlsRequired
        | ureq::Error::RequireHttpsOnly(_)
        | ureq::Error::Pem(_)
        | ureq::Error::Rustls(_) => NetworkKind::Tls,
        ureq::Error::HostNotFound
        | ureq::Error::ConnectionFailed
        | ureq::Error::ConnectProxyFailed(_) => NetworkKind::Connect,
        // Anything else (a bad URI, a protocol error, a decode failure, a
        // future variant) is not known to be transient.
        _ => NetworkKind::Other,
    }
}

/// The kind of an I/O error from the connection: `Timeout` for a timed-out
/// read, `Connect` only for connection-class kinds (refused, reset, aborted,
/// not connected, broken pipe, unreachable, address unavailable), `Tls` for
/// `InvalidData`, and `Other` for anything else.
///
/// ureq runs the rustls handshake through `ClientConnection::complete_io`,
/// which reports every rustls failure (an untrusted or expired certificate,
/// a name mismatch, a peer that does not speak TLS, an alert) as an
/// `io::Error` of kind `InvalidData`; nothing else in the request path
/// reports `InvalidData` (protocol faults are `ureq::Error::Protocol`), so the
/// kind is the TLS signal. `UnexpectedEof` is `Other`: the peer closed
/// mid-reply, after the request was sent.
fn io_error_kind(error: &std::io::Error) -> NetworkKind {
    use std::io::ErrorKind as Kind;
    match error.kind() {
        Kind::TimedOut => NetworkKind::Timeout,
        Kind::InvalidData => NetworkKind::Tls,
        Kind::ConnectionRefused
        | Kind::ConnectionReset
        | Kind::ConnectionAborted
        | Kind::NotConnected
        | Kind::BrokenPipe
        | Kind::HostUnreachable
        | Kind::NetworkUnreachable
        | Kind::NetworkDown
        | Kind::AddrNotAvailable => NetworkKind::Connect,
        _ => NetworkKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use sc_agent_types::ErrorId;

    use super::*;
    use crate::{Endpoint, Method};

    fn secret_request(key: &str) -> HttpRequest {
        HttpRequest::new(
            Method::Post,
            Endpoint::parse("https://x.invalid/").expect("https"),
            b"{}".as_slice(),
        )
        .with_secret(SecretHeader::new("x-api-key", crate::test_key::key(key)))
        .expect("https carries a key")
    }

    #[test]
    fn credential_header_is_sensitive_and_debug_hides_the_key() {
        let key = "sk-test-credential-123456";
        let request = secret_request(key);
        let outbound = build_outbound(&request).expect("built");
        let header = outbound.headers().get("x-api-key").expect("header");
        assert!(header.is_sensitive());
        assert_eq!(header.as_bytes(), key.as_bytes());
        let rendered = format!("{outbound:?}");
        assert!(!rendered.contains(key), "{rendered}");
    }

    #[test]
    fn invalid_header_is_invalid_request() {
        for (name, value) in [("bad name", "v"), ("x-ok", "bad\nvalue")] {
            let request = HttpRequest::new(
                Method::Get,
                Endpoint::parse("https://x.invalid/").expect("https"),
                Vec::new(),
            )
            .with_header(name, value)
            .expect("not a credential header");
            let error = build_outbound(&request).expect_err(name);
            assert_eq!(error.id(), "http.invalid_request", "{error:?}");
            let cause = error.cause_detail().expect("builder cause");
            assert_eq!(cause.id, "http.builder");
            assert!(!cause.message.is_empty());
        }
    }

    /// Only connection-class I/O kinds are `Connect`; a timeout is
    /// `Timeout`; any other I/O error is `Other`.
    #[test]
    fn io_errors_are_classified_by_what_happened() {
        use std::io::ErrorKind as Kind;
        for kind in [
            Kind::ConnectionRefused,
            Kind::ConnectionReset,
            Kind::BrokenPipe,
        ] {
            assert_eq!(
                io_error_kind(&std::io::Error::from(kind)),
                NetworkKind::Connect
            );
        }
        assert_eq!(
            io_error_kind(&std::io::Error::from(Kind::TimedOut)),
            NetworkKind::Timeout
        );
        for kind in [
            Kind::PermissionDenied,
            Kind::Other,
            Kind::OutOfMemory,
            Kind::UnexpectedEof,
        ] {
            assert_eq!(
                io_error_kind(&std::io::Error::from(kind)),
                NetworkKind::Other
            );
        }
        // `InvalidData` is how ureq surfaces a rustls failure (see
        // `io_error_kind`).
        assert_eq!(
            io_error_kind(&std::io::Error::from(Kind::InvalidData)),
            NetworkKind::Tls
        );
        let refused = ureq_error(ureq::Error::Io(std::io::Error::from(
            Kind::ConnectionRefused,
        )));
        assert!(matches!(
            refused,
            HttpError::Network {
                kind: NetworkKind::Connect,
                ..
            }
        ));
        let cause = refused.cause_detail().expect("io cause");
        assert_eq!(cause.id, "io.ConnectionRefused");
    }

    /// Why `ApiKey` owns the visible-ASCII rule: `http` itself accepts a
    /// non-ASCII character in a header value and refuses only controls.
    #[test]
    fn http_header_values_accept_non_ascii_but_not_controls() {
        assert!(ureq::http::HeaderValue::from_str("Bearer abc\u{e9}").is_ok());
        assert!(ureq::http::HeaderValue::from_str("Bearer abc\u{7f}").is_err());
        assert!(ureq::http::HeaderValue::from_str("Bearer abc\n").is_err());
    }

    #[test]
    fn request_timeout_is_global() {
        let timeouts = Timeouts::default().with_request(Duration::from_secs(300));
        let agent = AgentConfig::production(timeouts).build();
        let config = agent.config();
        assert_eq!(config.timeouts().connect, Some(Duration::from_secs(10)));
        assert_eq!(config.timeouts().global, Some(Duration::from_secs(300)));
        assert_eq!(config.max_redirects(), 0);
        assert!(!config.http_status_as_error());
        assert!(!config.https_only());
        assert_eq!(config.max_idle_connections_per_host(), MAX_IDLE_CONNECTIONS);
        assert_eq!(
            AgentConfig::production(Timeouts::default()),
            AgentConfig {
                connect: Duration::from_secs(10),
                global: Duration::from_secs(120),
                max_redirects: 0,
                user_agent: USER_AGENT,
            }
        );
        assert_eq!(
            USER_AGENT,
            format!("sc-agent-http/{}", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn ureq_errors_map_to_network_kinds() {
        let cases = [
            (
                ureq::Error::Timeout(ureq::Timeout::Global),
                NetworkKind::Timeout,
                "ureq.Timeout.Global",
            ),
            (
                ureq::Error::Io(std::io::Error::from(std::io::ErrorKind::TimedOut)),
                NetworkKind::Timeout,
                "io.TimedOut",
            ),
            (
                ureq::Error::Io(std::io::Error::from(std::io::ErrorKind::ConnectionReset)),
                NetworkKind::Connect,
                "io.ConnectionReset",
            ),
            (
                ureq::Error::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "invalid peer certificate",
                )),
                NetworkKind::Tls,
                "io.InvalidData",
            ),
            (ureq::Error::Tls("handshake"), NetworkKind::Tls, "ureq.Tls"),
            (
                ureq::Error::TlsRequired,
                NetworkKind::Tls,
                "ureq.TlsRequired",
            ),
            (
                ureq::Error::ConnectionFailed,
                NetworkKind::Connect,
                "ureq.ConnectionFailed",
            ),
            (
                ureq::Error::HostNotFound,
                NetworkKind::Connect,
                "ureq.HostNotFound",
            ),
            (
                ureq::Error::BadUri("x".to_owned()),
                NetworkKind::Other,
                "ureq.BadUri",
            ),
        ];
        for (error, kind, id) in cases {
            assert_eq!(ureq_error_kind(&error), kind, "{error}");
            let text = error.to_string();
            let mapped = ureq_error(error);
            let cause = mapped.cause_detail().expect("cause");
            assert_eq!(
                (cause.id.as_str(), cause.message.as_str()),
                (id, text.as_str())
            );
        }
        assert_eq!(
            ureq_error(ureq::Error::BodyExceedsLimit(1)).id(),
            "http.response_too_large"
        );
    }
}
