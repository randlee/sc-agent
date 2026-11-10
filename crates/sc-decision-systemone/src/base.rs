//! [`BaseUrl`]: a System One server's checked base URL, the one place endpoints are derived.

use std::fmt;

use sc_agent_http::{Endpoint, HttpError};

/// A server's base URL, parsed and checked once; every endpoint path is joined onto it here.
///
/// [`Self::https`] accepts https only (providers, TypeSafe); [`Self::local`] accepts http or
/// https (a configured local runner). One trailing `/` is dropped. A base carries no query
/// or fragment, so `{base}{path}` is always a well-formed URL.
///
/// # Examples
///
/// ```
/// use sc_decision_systemone::BaseUrl;
///
/// let base = BaseUrl::local("http://decider.internal:9000/")?;
/// assert_eq!(base.as_str(), "http://decider.internal:9000");
/// assert_eq!(base.join("/health")?.as_str(), "http://decider.internal:9000/health");
/// assert!(BaseUrl::https("http://api.example.test").is_err());
/// assert!(BaseUrl::local("http://h.test/?a=1").is_err());
/// # Ok::<(), sc_agent_http::HttpError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BaseUrl(Endpoint);

impl BaseUrl {
    /// An https base, e.g. [`TYPESAFE_BASE_URL`](crate::TYPESAFE_BASE_URL).
    ///
    /// # Errors
    ///
    /// [`HttpError::InvalidEndpoint`] when `url` is not an absolute `https://` URL with a host
    /// ([`Endpoint::parse`]), or has a query or fragment.
    pub fn https(url: &str) -> Result<Self, HttpError> {
        Self::parse(url, Endpoint::parse)
    }

    /// A configured local runner's base, e.g. `http://{host}:{port}`; http or https.
    ///
    /// # Errors
    ///
    /// [`HttpError::InvalidEndpoint`] when `url` is not an absolute `http://` or `https://`
    /// URL with a host ([`Endpoint::parse_local`]), or has a query or fragment.
    pub fn local(url: &str) -> Result<Self, HttpError> {
        Self::parse(url, Endpoint::parse_local)
    }

    fn parse(url: &str, parse: fn(&str) -> Result<Endpoint, HttpError>) -> Result<Self, HttpError> {
        let trimmed = url.strip_suffix('/').unwrap_or(url);
        let endpoint = parse(trimmed)?;
        if trimmed.contains(['?', '#']) {
            // log_form drops the query and fragment, so a credential in them is not kept.
            return Err(HttpError::InvalidEndpoint {
                url: endpoint.log_form(),
                reason: "a base URL has no query or fragment".to_owned(),
            });
        }
        Ok(Self(endpoint))
    }

    /// The base URL text, without a trailing `/`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// The endpoint `{base}{path}`; `path` starts with `/` (e.g. `/v1/systemone`).
    ///
    /// The scheme accepted when the base was built is kept: an https base gives an https
    /// endpoint.
    ///
    /// # Errors
    ///
    /// [`HttpError::InvalidEndpoint`] when `{base}{path}` does not parse (a `path` with
    /// characters a URL cannot hold).
    pub fn join(&self, path: &str) -> Result<Endpoint, HttpError> {
        Endpoint::parse_local(&format!("{}{path}", self.as_str()))
    }
}

impl fmt::Display for BaseUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
