//! Request and response values carried by a [`Transport`](crate::Transport).
//!
//! Every type here has private fields. Backends build an [`HttpRequest`];
//! transports build an [`HttpResponse`]. The credential is a
//! [`SecretHeader`], which no accessor in this module reveals.

use std::fmt;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use sc_agent_secrets::ApiKey;
use ureq::http::Uri;

use crate::error::HttpError;

/// HTTP request method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Method {
    /// `GET`.
    Get,
    /// `POST`. Every Phase A provider call uses it.
    Post,
}

impl Method {
    /// The method token as sent on the wire.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
        }
    }
}

impl fmt::Display for Method {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The credential header of one request.
///
/// Holds an [`ApiKey`]. Only [`UreqTransport`](crate::UreqTransport) writes
/// the value to the wire. Other transports, test doubles included, see the
/// name and [`Self::redacted`]. [`Self::key`] exists for redaction
/// ([`ApiKey::redact_text`]); [`Debug`](fmt::Debug) never prints the key.
///
/// # Examples
///
/// ```
/// # use sc_agent_secrets::{resolve_var, EnvSource, KeySources, KeyringSource, SecretsError};
/// # struct Env;
/// # impl EnvSource for Env { fn var(&self, _: &str) -> Option<String> { Some("sk-test".into()) } }
/// # struct NoKeyring;
/// # impl KeyringSource for NoKeyring {
/// #     fn password(&self, _: &str, _: &str) -> Result<Option<String>, SecretsError> { Ok(None) }
/// # }
/// # let sources = KeySources { env: &Env, repo_root: None, keyring: &NoKeyring };
/// # let key = resolve_var(&"TEST_KEY".parse()?, &sources)?.key;
/// use sc_agent_http::SecretHeader;
///
/// let header = SecretHeader::bearer(key);
/// assert_eq!(header.name(), "Authorization");
/// assert_eq!(header.redacted(), "Authorization: Bearer [redacted]");
/// assert!(!format!("{header:?}").contains("sk-test"));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone)]
pub struct SecretHeader {
    name: &'static str,
    prefix: &'static str,
    key: ApiKey,
}

impl SecretHeader {
    /// A header named `name` whose value is the key itself (for example
    /// `x-api-key`).
    #[must_use]
    pub fn new(name: &'static str, key: ApiKey) -> Self {
        Self {
            name,
            prefix: "",
            key,
        }
    }

    /// `Authorization: Bearer <key>`.
    #[must_use]
    pub fn bearer(key: ApiKey) -> Self {
        Self {
            name: "Authorization",
            prefix: "Bearer ",
            key,
        }
    }

    /// The header name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The key, for redaction only ([`ApiKey::redact_text`]).
    #[must_use]
    pub fn key(&self) -> &ApiKey {
        &self.key
    }

    /// `<name>: <scheme prefix>[redacted]`, safe to log.
    #[must_use]
    pub fn redacted(&self) -> String {
        format!("{}: {}[redacted]", self.name, self.prefix)
    }

    /// Text inserted before the key in the header value (`"Bearer "` or empty).
    pub(crate) fn prefix(&self) -> &'static str {
        self.prefix
    }
}

impl fmt::Debug for SecretHeader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretHeader")
            .field("name", &self.name)
            .field("value", &format_args!("{}[redacted]", self.prefix))
            .finish()
    }
}

/// An absolute URL a request may be sent to.
///
/// [`Endpoint::parse`] accepts only `https://` URLs: every provider is
/// reached over TLS. [`Endpoint::parse_local`] also accepts `http://`, for a
/// configured local runner (sc-decision-local), which often has no TLS; a key
/// may still be attached only when its host is loopback
/// ([`HttpRequest::with_secret`], REQ-SAG-0013).
///
/// # Examples
///
/// ```
/// use sc_agent_http::Endpoint;
///
/// let endpoint = Endpoint::parse("https://api.openai.com/v1/responses")?;
/// assert_eq!(endpoint.as_str(), "https://api.openai.com/v1/responses");
/// assert_eq!(endpoint.host(), "api.openai.com");
/// assert!(Endpoint::parse("http://api.openai.com/v1/responses").is_err());
///
/// let local = Endpoint::parse_local("http://127.0.0.1:11434/v1/chat/completions")?;
/// assert!(local.is_plain_http() && local.is_loopback());
/// # Ok::<(), sc_agent_http::HttpError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Endpoint {
    url: String,
}

/// Which schemes a parse accepts.
#[derive(Clone, Copy)]
enum Schemes {
    HttpsOnly,
    HttpOrHttps,
}

impl Endpoint {
    /// Parses an absolute `https://` URL with a host.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::InvalidEndpoint`] when `url` is not a valid URL,
    /// has no host, or its scheme is not `https`.
    pub fn parse(url: &str) -> Result<Self, HttpError> {
        Self::parse_with(url, Schemes::HttpsOnly)
    }

    /// Parses an absolute `http://` or `https://` URL with a host.
    ///
    /// Only for the endpoint of a configured local runner (a `local_runners`
    /// entry); providers use [`Self::parse`].
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::InvalidEndpoint`] when `url` is not a valid URL,
    /// has no host, or its scheme is neither `http` nor `https`.
    pub fn parse_local(url: &str) -> Result<Self, HttpError> {
        Self::parse_with(url, Schemes::HttpOrHttps)
    }

    fn parse_with(url: &str, schemes: Schemes) -> Result<Self, HttpError> {
        let invalid = |reason: String| HttpError::InvalidEndpoint {
            url: sanitize_url(url),
            reason,
        };
        let uri = Uri::from_str(url).map_err(|error| invalid(error.to_string()))?;
        // The scheme is compared on the text as written, lower case only.
        let scheme = url.split_once("://").map(|(scheme, _)| scheme);
        let allowed = match schemes {
            Schemes::HttpsOnly => scheme == Some("https"),
            Schemes::HttpOrHttps => matches!(scheme, Some("https" | "http")),
        };
        if !allowed {
            return Err(invalid(match schemes {
                Schemes::HttpsOnly => "the scheme must be https".to_owned(),
                Schemes::HttpOrHttps => "the scheme must be http or https".to_owned(),
            }));
        }
        if uri.host().is_none_or(str::is_empty) {
            return Err(invalid("the URL has no host".to_owned()));
        }
        Ok(Self {
            url: url.to_owned(),
        })
    }

    /// The URL text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.url
    }

    /// The endpoint as it may appear in a log: scheme, host and path only.
    ///
    /// Userinfo (`user:pass@`), the port, the query and the fragment are
    /// dropped, so a credential carried in a URL cannot reach a log. This is
    /// the one form logs use.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_http::Endpoint;
    ///
    /// let endpoint = Endpoint::parse("https://user:pw@api.example.test:8443/v1/x?key=SECRET#f")?;
    /// assert_eq!(endpoint.log_form(), "https://api.example.test/v1/x");
    /// # Ok::<(), sc_agent_http::HttpError>(())
    /// ```
    #[must_use]
    pub fn log_form(&self) -> String {
        let parts = self.parts();
        format!("{}://{}{}", parts.scheme, parts.host, parts.path)
    }

    /// The host without port or userinfo (IPv6 in brackets): the
    /// [`ProviderPause`](crate::ProviderPause) key.
    #[must_use]
    pub fn host(&self) -> &str {
        self.parts().host
    }

    /// Scheme is http (only [`Self::parse_local`] can produce one).
    #[must_use]
    pub fn is_plain_http(&self) -> bool {
        self.parts().scheme == "http"
    }

    /// Host is `localhost`, an IPv4 address in 127.0.0.0/8, or `::1`
    /// (parsed with `std::net`, no DNS lookup).
    #[must_use]
    pub fn is_loopback(&self) -> bool {
        let host = self.host();
        if host.eq_ignore_ascii_case("localhost") {
            return true;
        }
        let bare = host
            .strip_prefix('[')
            .and_then(|inner| inner.strip_suffix(']'))
            .unwrap_or(host);
        match IpAddr::from_str(bare) {
            Ok(IpAddr::V4(address)) => address.is_loopback(),
            Ok(IpAddr::V6(address)) => address == std::net::Ipv6Addr::LOCALHOST,
            Err(_) => false,
        }
    }

    /// Scheme, host and path of the URL text (validated at parse).
    fn parts(&self) -> UrlParts<'_> {
        let text = self.url.as_str();
        let (scheme, rest) = text.split_once("://").unwrap_or(("", text));
        let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let (authority, tail) = rest.split_at_checked(end).unwrap_or((rest, ""));
        let host_port = authority
            .rsplit_once('@')
            .map_or(authority, |(_, host)| host);
        let host = if host_port.starts_with('[') {
            host_port
                .split_once(']')
                .and_then(|(inner, _)| host_port.get(..=inner.len()))
                .unwrap_or(host_port)
        } else {
            host_port.split(':').next().unwrap_or(host_port)
        };
        let path = tail.split(['?', '#']).next().unwrap_or("");
        UrlParts { scheme, host, path }
    }
}

/// Longest URL text kept in an [`HttpError::InvalidEndpoint`], in bytes.
const INVALID_URL_MAX_BYTES: usize = 256;

/// `url` safe to carry in an error: userinfo (`user:pass@`), the query and
/// the fragment are dropped, and the rest is cut to
/// [`INVALID_URL_MAX_BYTES`] at a character boundary (ending in `...`).
/// Works on text that does not parse.
fn sanitize_url(url: &str) -> String {
    let (scheme, rest) = match url.split_once("://") {
        Some((scheme, rest)) => (Some(scheme), rest),
        None => (None, url),
    };
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    // Everything up to the last '@' goes, even past a '/': a malformed
    // userinfo with a '/' in its password must not leak half of it.
    let rest = rest.rsplit_once('@').map_or(rest, |(_, after)| after);
    let mut text = match scheme {
        Some(scheme) => format!("{scheme}://{rest}"),
        None => rest.to_owned(),
    };
    if text.len() > INVALID_URL_MAX_BYTES {
        let mut cut = INVALID_URL_MAX_BYTES - 3;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
        text.push_str("...");
    }
    text
}

/// Header names that carry credentials: refused by
/// [`HttpRequest::with_header`] (compared ASCII case-insensitively), so a key
/// enters a request only through [`HttpRequest::with_secret`], where it is
/// redacted and guarded (REQ-SAG-0013).
const CREDENTIAL_HEADERS: [&str; 6] = [
    "authorization",
    "proxy-authorization",
    "x-api-key",
    "api-key",
    "x-goog-api-key",
    "cookie",
];

/// Borrowed pieces of an [`Endpoint`] URL.
struct UrlParts<'a> {
    scheme: &'a str,
    host: &'a str,
    path: &'a str,
}

impl fmt::Display for Endpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.url)
    }
}

/// One outbound HTTP request.
///
/// The body is shared bytes (`Arc<[u8]>`), so multipart bodies can be sent
/// and resends send the same bytes without copying. [`Debug`](fmt::Debug)
/// prints the method, the endpoint's log form, the header names, the body
/// length and the redacted secret: never a header value, the body or the key.
///
/// # Examples
///
/// ```
/// # use sc_agent_secrets::{resolve_var, EnvSource, KeySources, KeyringSource, SecretsError};
/// # struct Env;
/// # impl EnvSource for Env { fn var(&self, _: &str) -> Option<String> { Some("sk-test".into()) } }
/// # struct NoKeyring;
/// # impl KeyringSource for NoKeyring {
/// #     fn password(&self, _: &str, _: &str) -> Result<Option<String>, SecretsError> { Ok(None) }
/// # }
/// # let sources = KeySources { env: &Env, repo_root: None, keyring: &NoKeyring };
/// # let key = resolve_var(&"TEST_KEY".parse()?, &sources)?.key;
/// use sc_agent_http::{Endpoint, HttpRequest, Method, SecretHeader};
///
/// let request = HttpRequest::new(
///     Method::Post,
///     Endpoint::parse("https://api.anthropic.com/v1/messages")?,
///     br#"{"model":"m"}"#.as_slice(),
/// )
/// .with_header("content-type", "application/json")?
/// .with_secret(SecretHeader::new("x-api-key", key))?;
/// assert_eq!(request.headers().len(), 1);
/// assert_eq!(request.body(), br#"{"model":"m"}"#);
/// assert!(!format!("{request:?}").contains("sk-test"));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone)]
pub struct HttpRequest {
    method: Method,
    endpoint: Endpoint,
    headers: Vec<(String, String)>,
    secret: Option<SecretHeader>,
    body: Arc<[u8]>,
    deadline: Option<Duration>,
}

impl HttpRequest {
    /// A request with no headers, no secret and no per-request deadline.
    #[must_use]
    pub fn new(method: Method, endpoint: Endpoint, body: impl Into<Arc<[u8]>>) -> Self {
        Self {
            method,
            endpoint,
            headers: Vec::new(),
            secret: None,
            body: body.into(),
            deadline: None,
        }
    }

    /// Appends a non-secret header. Headers are sent in insertion order.
    ///
    /// Other name and value syntax is checked when the transport builds the
    /// request: one the HTTP builder refuses is [`HttpError::InvalidRequest`]
    /// at send.
    ///
    /// # Errors
    ///
    /// [`HttpError::InvalidRequest`] when `name` is a credential header
    /// (`authorization`, `proxy-authorization`, `x-api-key`, `api-key`,
    /// `x-goog-api-key`, `cookie`, any case): a key goes only through
    /// [`Self::with_secret`]. The error never contains the value.
    pub fn with_header(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, HttpError> {
        let name = name.into();
        if CREDENTIAL_HEADERS
            .iter()
            .any(|credential| name.eq_ignore_ascii_case(credential))
        {
            return Err(HttpError::invalid_request(format!(
                "{name:?} is a credential header: attach the key with with_secret"
            )));
        }
        self.headers.push((name, value.into()));
        Ok(self)
    }

    /// Sets the credential header, replacing any earlier one.
    ///
    /// This is the only way a secret enters a request, so the REQ-SAG-0013
    /// check runs here, before any send: no request to a plain-http host
    /// that is not loopback carries a key.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::KeyOnPlainHttp`] when the endpoint is plain http
    /// ([`Endpoint::is_plain_http`]) and its host is not loopback
    /// ([`Endpoint::is_loopback`]).
    pub fn with_secret(mut self, secret: SecretHeader) -> Result<Self, HttpError> {
        if self.endpoint.is_plain_http() && !self.endpoint.is_loopback() {
            return Err(HttpError::KeyOnPlainHttp {
                host: self.endpoint.host().to_owned(),
            });
        }
        self.secret = Some(secret);
        Ok(self)
    }

    /// Replaces the transport's request timeout
    /// ([`Timeouts::request`](crate::Timeouts::request)) for every send of
    /// this request only (REQ-SAG-0015): a load-on-first-request daemon's
    /// first request gets the load timeout plus the request timeout. Resends
    /// keep it unchanged.
    ///
    /// The deadline must be in `(0, Timeouts::DEADLINE_MAX]` (three hours:
    /// the largest load timeout plus the largest request timeout); otherwise
    /// [`HttpClient::send_once`](crate::HttpClient::send_once) and
    /// [`UreqTransport`](crate::UreqTransport) refuse the request with
    /// [`HttpError::InvalidRequest`] before any send.
    #[must_use]
    pub fn with_deadline(mut self, deadline: Duration) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// The per-request deadline, when [`Self::with_deadline`] set one.
    #[must_use]
    pub fn deadline(&self) -> Option<Duration> {
        self.deadline
    }

    /// The method.
    #[must_use]
    pub fn method(&self) -> Method {
        self.method
    }

    /// The target URL.
    #[must_use]
    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    /// Non-secret headers `(name, value)`, in send order.
    #[must_use]
    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    /// The credential header, if any.
    #[must_use]
    pub fn secret(&self) -> Option<&SecretHeader> {
        self.secret.as_ref()
    }

    /// The body bytes; transports send them unchanged on every send.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

impl fmt::Debug for HttpRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let header_names: Vec<&str> = self.headers.iter().map(|(name, _)| name.as_str()).collect();
        formatter
            .debug_struct("HttpRequest")
            .field("method", &self.method)
            .field("endpoint", &self.endpoint.log_form())
            .field("headers", &header_names)
            .field("secret", &self.secret.as_ref().map(SecretHeader::redacted))
            .field("body_len", &self.body.len())
            .field("deadline", &self.deadline)
            .finish()
    }
}
/// One HTTP response, including non-2xx statuses.
///
/// [`Transport`](crate::Transport) implementations build it.
///
/// # Examples
///
/// ```
/// use sc_agent_http::HttpResponse;
///
/// let response = HttpResponse::new(
///     429,
///     vec![("Retry-After".to_owned(), "2".to_owned())],
///     b"busy".to_vec(),
/// );
/// assert_eq!(response.header("retry-after"), Some("2"));
/// assert_eq!(response.into_body(), b"busy");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl HttpResponse {
    /// A response with `status`, `headers` (name, value) and `body`.
    #[must_use]
    pub fn new(status: u16, headers: Vec<(String, String)>, body: Vec<u8>) -> Self {
        Self {
            status,
            headers,
            body,
        }
    }

    /// The HTTP status code.
    #[must_use]
    pub fn status(&self) -> u16 {
        self.status
    }

    /// The first header named `name`, compared ASCII case-insensitively.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// The body bytes.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Consumes the response and returns the body.
    #[must_use]
    pub fn into_body(self) -> Vec<u8> {
        self.body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            assert!(
                matches!(error, HttpError::InvalidEndpoint { .. }),
                "{url}: {error:?}"
            );
        }
        let endpoint = Endpoint::parse("https://api.openai.com/v1/responses").expect("https");
        assert!(!endpoint.is_plain_http());
    }

    #[test]
    fn parse_local_accepts_http() {
        for url in [
            "http://127.0.0.1:11434/v1/chat/completions",
            "http://localhost:8080/",
            "http://10.0.0.5/v1",
            "https://gpu.lan/v1",
        ] {
            let endpoint = Endpoint::parse_local(url).expect(url);
            assert_eq!(endpoint.as_str(), url);
        }
        for url in ["ftp://host/", "http://", "localhost:8080/x", "HTTP://host/"] {
            assert!(
                matches!(
                    Endpoint::parse_local(url),
                    Err(HttpError::InvalidEndpoint { .. })
                ),
                "{url}"
            );
        }
    }

    #[test]
    fn host_and_loopback_need_no_lookup() {
        let cases = [
            ("http://127.0.0.1:8080/x", "127.0.0.1", true),
            ("http://127.9.8.7/x", "127.9.8.7", true),
            ("http://LocalHost:1/", "LocalHost", true),
            ("http://[::1]:8080/x", "[::1]", true),
            ("http://u:p@localhost/", "localhost", true),
            ("http://10.0.0.5/v1", "10.0.0.5", false),
            ("http://[::2]/", "[::2]", false),
            ("http://localhost.evil.test/", "localhost.evil.test", false),
            (
                "https://api.example.test:8443/v1?q#f",
                "api.example.test",
                false,
            ),
        ];
        for (url, host, loopback) in cases {
            let endpoint = Endpoint::parse_local(url).expect(url);
            assert_eq!(endpoint.host(), host, "{url}");
            assert_eq!(endpoint.is_loopback(), loopback, "{url}");
        }
        assert_eq!(
            Endpoint::parse_local("http://[::1]:8080/x?k=v")
                .expect("v6")
                .log_form(),
            "http://[::1]/x"
        );
    }

    #[test]
    fn invalid_endpoint_drops_userinfo_query_and_fragment() {
        for url in [
            "http://user:pw-SECRET@api.example.test/v1?key=SECRET#SECRET",
            "ftp://user:pw-SECRET@host/x?token=SECRET",
            "user:pw-SECRET@host/x?SECRET",
            "https://user:pw-SECRET@[::1/x?SECRET",
            "ftp://user:pw/SECRET@host/x",
        ] {
            let error = Endpoint::parse(url).expect_err(url);
            let rendered = format!(
                "{error} {error:?} {:?}",
                sc_agent_types::ErrorId::detail(&error)
            );
            assert!(!rendered.contains("SECRET"), "{url}: {rendered}");
            assert!(!rendered.contains("user:pw"), "{url}: {rendered}");
        }
        let long = format!("ftp://host/{}", "é".repeat(400));
        match Endpoint::parse(&long) {
            Err(HttpError::InvalidEndpoint { url, .. }) => {
                assert!(
                    url.len() <= INVALID_URL_MAX_BYTES && url.ends_with("..."),
                    "{url}"
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn credential_headers_are_refused() {
        let request = || {
            HttpRequest::new(
                Method::Get,
                Endpoint::parse("https://h.test/").expect("https"),
                Vec::new(),
            )
        };
        for name in [
            "Authorization",
            "PROXY-AUTHORIZATION",
            "x-api-key",
            "Api-Key",
            "X-Goog-Api-Key",
            "cookie",
        ] {
            let error = request().with_header(name, "sk-VALUE").expect_err(name);
            assert!(matches!(error, HttpError::InvalidRequest { .. }), "{name}");
            assert!(!format!("{error:?}").contains("sk-VALUE"));
        }
        let ok = request()
            .with_header("x-api-version", "1")
            .expect("not a credential");
        assert_eq!(ok.headers().len(), 1);
    }

    #[test]
    fn debug_prints_no_header_value_or_body() {
        let request = HttpRequest::new(
            Method::Post,
            Endpoint::parse("https://u:pw@api.example.test/v1?key=q").expect("https"),
            b"secret-body".as_slice(),
        )
        .with_header("x-trace", "header-value")
        .expect("not a credential header");
        let rendered = format!("{request:?}");
        for hidden in ["header-value", "secret-body", "pw", "key=q"] {
            assert!(!rendered.contains(hidden), "{hidden}: {rendered}");
        }
        assert!(rendered.contains("x-trace") && rendered.contains("body_len: 11"));
    }
}
