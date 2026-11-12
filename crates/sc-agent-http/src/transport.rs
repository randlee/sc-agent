//! The [`Transport`] trait and transport [`Timeouts`].

use std::sync::Arc;
use std::time::Duration;

use crate::error::HttpError;
use crate::request::{HttpRequest, HttpResponse};

/// Sends one HTTP request, once.
///
/// Open trait: test doubles and callers' own transports implement it
/// (ADR-SAG-0001). Object-safe and `Send + Sync`.
///
/// Calling `send` directly bypasses the busy and quota classification, the
/// resend schedule and redaction; production sends go through
/// [`HttpClient::send_once`](crate::HttpClient::send_once). A transport
/// other than [`UreqTransport`](crate::UreqTransport) cannot read the key in
/// [`HttpRequest::secret`]; it sees only the header name and
/// [`SecretHeader::redacted`](crate::SecretHeader::redacted).
pub trait Transport: Send + Sync {
    /// Sends `request` and returns the response, whatever its status.
    ///
    /// # Errors
    ///
    /// Every HTTP status is `Ok`; only transport failures are `Err`:
    /// [`HttpError::Network`] when the connection, the send or the body read
    /// fails or times out, [`HttpError::ResponseTooLarge`] when the body
    /// exceeds the transport cap, [`HttpError::InvalidRequest`] when a header
    /// cannot be built.
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError>;

    /// Checks that this transport can send at all (for example, that its timeouts are in
    /// bounds), before anything is logged or counted.
    /// [`HttpClient::send_once`](crate::HttpClient::send_once) calls it first, so a transport
    /// that would refuse every send never logs `http.send` or uses up an attempt. The
    /// provided body accepts.
    ///
    /// # Errors
    ///
    /// [`HttpError::InvalidRequest`] when the transport cannot send.
    fn ready(&self) -> Result<(), HttpError> {
        Ok(())
    }
}

impl<T: Transport + ?Sized> Transport for &T {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError> {
        (**self).send(request)
    }

    fn ready(&self) -> Result<(), HttpError> {
        (**self).ready()
    }
}

impl<T: Transport + ?Sized> Transport for Arc<T> {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError> {
        (**self).send(request)
    }

    fn ready(&self) -> Result<(), HttpError> {
        (**self).ready()
    }
}

/// Connect and whole-request deadlines of a transport.
///
/// `connect`: TCP + TLS connect (ureq `timeout_connect`). `request`: one
/// global deadline for the whole send, connect, headers and body read
/// included (ureq `timeout_global`), so a slow-drip body cannot exceed it.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
///
/// use sc_agent_http::Timeouts;
///
/// let timeouts = Timeouts::default();
/// assert_eq!(timeouts.connect, Duration::from_secs(10));
/// assert_eq!(timeouts.request, Duration::from_secs(120));
/// let runner = timeouts.with_request(Duration::from_secs(300));
/// assert_eq!((runner.connect, runner.request), (Duration::from_secs(10), Duration::from_secs(300)));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Timeouts {
    /// TCP + TLS connect deadline.
    pub connect: Duration,
    /// Deadline for the whole send, connect, headers and body read included.
    pub request: Duration,
}

impl Default for Timeouts {
    /// Connect 10 s, request 120 s: decision and LLM sends.
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(10),
            request: Duration::from_secs(120),
        }
    }
}

impl Timeouts {
    /// Longest accepted connect or request timeout: one hour (the largest
    /// configurable request timeout; a typo such as a value in milliseconds
    /// does not fit).
    pub const MAX: Duration = Duration::from_secs(3600);

    /// Longest accepted per-request deadline
    /// ([`HttpRequest::with_deadline`](crate::HttpRequest::with_deadline)):
    /// three hours, the largest load timeout (7200 s) plus the largest
    /// request timeout (3600 s) for a load-on-first-request daemon's first
    /// request (REQ-SAG-0015).
    pub const DEADLINE_MAX: Duration = Duration::from_secs(10_800);

    /// `self` when both timeouts are in `(0, MAX]`.
    ///
    /// [`UreqTransport`](crate::UreqTransport) runs this check at each send;
    /// callers that build `Timeouts` from config may run it at load.
    ///
    /// # Errors
    ///
    /// [`HttpError::InvalidRequest`] when `connect` or `request` is zero or
    /// longer than [`Self::MAX`].
    pub fn validate(self) -> Result<Self, HttpError> {
        check_duration("the connect timeout", self.connect, Self::MAX)?;
        check_duration("the request timeout", self.request, Self::MAX)?;
        Ok(self)
    }

    /// Connect 5 s, request 10 s: discovery, `decidr models`, `decidr doctor`
    /// and health probes.
    #[must_use]
    pub const fn probe() -> Self {
        Self {
            connect: Duration::from_secs(5),
            request: Duration::from_secs(10),
        }
    }

    /// Same connect timeout, this request deadline (REQ-SAG-0015: a local
    /// runner's own request timeout; one `UreqTransport` per local runner is
    /// built with `Timeouts::default().with_request(runner.request_timeout)`).
    #[must_use]
    pub const fn with_request(self, request: Duration) -> Self {
        Self {
            connect: self.connect,
            request,
        }
    }
}

/// `Ok` when `value` is in `(0, max]`; `what` names it in the error.
pub(crate) fn check_duration(what: &str, value: Duration, max: Duration) -> Result<(), HttpError> {
    if value.is_zero() {
        return Err(HttpError::invalid_request(format!("{what} is zero")));
    }
    if value > max {
        return Err(HttpError::invalid_request(format!(
            "{what} of {value:?} is over the {max:?} maximum"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_timeouts() {
        let probe = Timeouts::probe();
        assert_eq!(
            (probe.connect, probe.request),
            (Duration::from_secs(5), Duration::from_secs(10))
        );
    }

    #[test]
    fn zero_and_over_long_timeouts_are_refused() {
        assert!(Timeouts::default().validate().is_ok());
        assert!(
            Timeouts::default()
                .with_request(Timeouts::MAX)
                .validate()
                .is_ok()
        );
        for bad in [
            Timeouts::default().with_request(Duration::ZERO),
            Timeouts::default().with_request(Timeouts::MAX + Duration::from_nanos(1)),
            Timeouts {
                connect: Duration::ZERO,
                request: Duration::from_secs(1),
            },
        ] {
            assert!(
                matches!(bad.validate(), Err(HttpError::InvalidRequest { .. })),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn timeouts_with_request() {
        let base = Timeouts::probe();
        let changed = base.with_request(Duration::from_secs(300));
        assert_eq!(changed.connect, base.connect);
        assert_eq!(changed.request, Duration::from_secs(300));
    }
}
