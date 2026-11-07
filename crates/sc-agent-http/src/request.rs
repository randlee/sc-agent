//! Request and response values carried by a [`Transport`](crate::Transport).
//!
//! Every type here has private fields. Backends build an [`HttpRequest`];
//! transports build an [`HttpResponse`]. The credential is a
//! [`SecretHeader`], which no accessor in this module reveals.

use std::fmt;
use std::sync::Arc;

use sc_agent_types::{JevError, ResolvedApiKey};

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

/// A header name that is a valid RFC 9110 token.
///
/// Built from a `&'static str`, so header names are compile-time constants
/// of the backend crate, not runtime input.
///
/// # Examples
///
/// ```
/// use sc_agent_http::HeaderName;
///
/// const VERSION: HeaderName = HeaderName::new("anthropic-version");
/// assert_eq!(VERSION.as_str(), "anthropic-version");
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct HeaderName(&'static str);

impl HeaderName {
    /// Validates `name` as an RFC 9110 token.
    ///
    /// # Panics
    ///
    /// Panics when `name` is empty or holds a byte outside the token set
    /// (ASCII letters, digits and ``!#$%&'*+-.^_`|~``). In a `const` item the
    /// panic is a compile error.
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        let bytes = name.as_bytes();
        assert!(!bytes.is_empty(), "header name is empty");
        let mut index = 0;
        while index < bytes.len() {
            assert!(
                is_token_byte(bytes[index]),
                "header name is not an RFC 9110 token"
            );
            index += 1;
        }
        Self(name)
    }

    /// The name as given.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// RFC 9110 `tchar`.
const fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

impl fmt::Debug for HeaderName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.0, formatter)
    }
}

impl fmt::Display for HeaderName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

/// The credential header of one request.
///
/// Holds a [`ResolvedApiKey`]. Only [`UreqTransport`](crate::UreqTransport)
/// writes the value to the wire (ADR-SAG-0020). Other transports, test
/// doubles included, see the name, [`Self::redacted`] and
/// [`Self::matches`]; nothing here returns the key.
///
/// # Examples
///
/// ```
/// use sc_agent_http::{HeaderName, SecretHeader};
/// use sc_agent_types::{BackendKeyId, ResolvedApiKey};
///
/// let key = ResolvedApiKey::try_new(BackendKeyId::OpenAi, "sk-test".to_owned())?;
/// let header = SecretHeader::bearer(key);
/// assert_eq!(header.name().as_str(), "Authorization");
/// assert_eq!(header.redacted(), "Authorization: Bearer [redacted]");
/// assert!(header.matches("sk-test"));
/// # Ok::<(), sc_agent_types::JevError>(())
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct SecretHeader {
    name: HeaderName,
    prefix: &'static str,
    value: ResolvedApiKey,
}

impl SecretHeader {
    /// A header whose value is the key itself (for example `x-api-key`).
    #[must_use]
    pub fn new(name: HeaderName, value: ResolvedApiKey) -> Self {
        Self {
            name,
            prefix: "",
            value,
        }
    }

    /// `Authorization: Bearer <key>`.
    #[must_use]
    pub fn bearer(value: ResolvedApiKey) -> Self {
        Self {
            name: HeaderName::new("Authorization"),
            prefix: "Bearer ",
            value,
        }
    }

    /// The header name.
    #[must_use]
    pub fn name(&self) -> HeaderName {
        self.name
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

    /// The sealed key. Read only through `ureq_transport`.
    pub(crate) fn key(&self) -> &ResolvedApiKey {
        &self.value
    }
}

impl fmt::Debug for SecretHeader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretHeader")
            .field("name", &self.name)
            .field("value", &self.value)
            .finish()
    }
}

/// An absolute URL a request may be sent to.
///
/// [`Endpoint::parse`] accepts only `https://` URLs, in every build. With the
/// `test-util` feature, `Endpoint::loopback_http` can also build
/// `http://127.0.0.1:<port>/…`, and nothing else in plain HTTP.
///
/// # Examples
///
/// ```
/// use sc_agent_http::Endpoint;
///
/// let endpoint = Endpoint::parse("https://api.openai.com/v1/responses")?;
/// assert_eq!(endpoint.as_str(), "https://api.openai.com/v1/responses");
/// assert!(Endpoint::parse("http://api.openai.com/v1/responses").is_err());
/// # Ok::<(), sc_agent_types::JevError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Endpoint {
    url: String,
    https: bool,
}

impl Endpoint {
    /// Parses an absolute `https://` URL with a host.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::ConfigInvalid`] when `url` is not a valid URL, has
    /// no host, or its scheme is not `https`.
    pub fn parse(url: &str) -> Result<Self, JevError> {
        crate::ureq_transport::check_https_url(url)?;
        Ok(Self {
            url: url.to_owned(),
            https: true,
        })
    }

    /// `http://127.0.0.1:<port><path>`, for loopback transport tests.
    ///
    /// Only the IPv4 loopback address can be built, so this cannot send plain
    /// HTTP off the machine even if a build enables `test-util` by mistake.
    ///
    /// # Panics
    ///
    /// Panics when `path` does not start with `/` or is not a valid URL path.
    #[cfg(feature = "test-util")]
    #[must_use]
    pub fn loopback_http(port: u16, path: &str) -> Self {
        let url = format!("http://127.0.0.1:{port}{path}");
        assert!(path.starts_with('/'), "loopback path must start with '/'");
        assert!(
            crate::ureq_transport::is_valid_url(&url),
            "loopback path is not a valid URL path"
        );
        Self { url, https: false }
    }

    /// The URL text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.url
    }

    /// Whether the scheme is `https` (always true outside `test-util`).
    #[must_use]
    pub fn is_https(&self) -> bool {
        self.https
    }
}

impl fmt::Display for Endpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.url)
    }
}

/// One outbound HTTP request.
///
/// The body is shared (`Arc<[u8]>`), so busy retries resend the same bytes
/// without copying. [`Debug`](fmt::Debug) prints the body length and redacts
/// the secret.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
///
/// use sc_agent_http::{Endpoint, HeaderName, HttpRequest, Method, SecretHeader};
/// use sc_agent_types::{BackendKeyId, ResolvedApiKey};
///
/// let key = ResolvedApiKey::try_new(BackendKeyId::Anthropic, "sk-test".to_owned())?;
/// let request = HttpRequest::new(
///     Method::Post,
///     Endpoint::parse("https://api.anthropic.com/v1/messages")?,
///     Arc::from(&br#"{"model":"m"}"#[..]),
/// )
/// .with_header(HeaderName::new("content-type"), "application/json")
/// .with_secret(SecretHeader::new(HeaderName::new("x-api-key"), key));
/// assert_eq!(request.headers().len(), 1);
/// assert!(!format!("{request:?}").contains("sk-test"));
/// # Ok::<(), sc_agent_types::JevError>(())
/// ```
#[derive(Clone)]
pub struct HttpRequest {
    method: Method,
    endpoint: Endpoint,
    headers: Vec<(HeaderName, String)>,
    secret: Option<SecretHeader>,
    body: Arc<[u8]>,
}

impl HttpRequest {
    /// A request with no headers and no secret.
    #[must_use]
    pub fn new(method: Method, endpoint: Endpoint, body: Arc<[u8]>) -> Self {
        Self {
            method,
            endpoint,
            headers: Vec::new(),
            secret: None,
            body,
        }
    }

    /// Appends a non-secret header. Headers are sent in insertion order.
    #[must_use]
    pub fn with_header(mut self, name: HeaderName, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }

    /// Sets the credential header, replacing any earlier one.
    #[must_use]
    pub fn with_secret(mut self, secret: SecretHeader) -> Self {
        self.secret = Some(secret);
        self
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

    /// Non-secret headers, in send order.
    #[must_use]
    pub fn headers(&self) -> &[(HeaderName, String)] {
        &self.headers
    }

    /// The credential header, if any.
    #[must_use]
    pub fn secret(&self) -> Option<&SecretHeader> {
        self.secret.as_ref()
    }

    /// The body bytes sent on every attempt.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

impl fmt::Debug for HttpRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpRequest")
            .field("method", &self.method)
            .field("endpoint", &self.endpoint)
            .field("headers", &self.headers)
            .field("secret", &self.secret)
            .field("body_len", &self.body.len())
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
    fn token_bytes() {
        for byte in b"aZ09!#$%&'*+-.^_`|~" {
            assert!(is_token_byte(*byte), "{}", *byte as char);
        }
        for byte in b" :/()<>@,;=\"[]?{}\x7f\r\n" {
            assert!(!is_token_byte(*byte), "{byte}");
        }
    }

    #[test]
    #[should_panic(expected = "not an RFC 9110 token")]
    fn header_name_rejects_separators() {
        let _ = HeaderName::new("bad header");
    }
}
