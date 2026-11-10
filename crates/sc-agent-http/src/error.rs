//! [`HttpError`], the one error type of sc-agent-http, and its two kinds
//! [`BusyReason`] and [`NetworkKind`] (REQ-SAG-0002).

use std::fmt;

use sc_agent_types::{ErrorDetail, ErrorId};

use crate::reply::ProviderReply;

/// Why a held busy request was not sent again (`http.provider_busy`).
///
/// # Examples
///
/// ```
/// use sc_agent_http::BusyReason;
///
/// assert_eq!(BusyReason::Sends.as_str(), "sends");
/// assert_eq!(BusyReason::Budget.to_string(), "budget");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BusyReason {
    /// The last allowed send (`max_sends`) was busy.
    Sends,
    /// The 90 s start budget, measured from the first send, has run out.
    Budget,
}

impl BusyReason {
    /// The snake_case spelling (`sends`, `budget`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sends => "sends",
            Self::Budget => "budget",
        }
    }
}

impl fmt::Display for BusyReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Transport failure class of an [`HttpError::Network`].
///
/// A transport failure is one where no usable HTTP reply arrived. A reply
/// with a failing status is never a `NetworkKind`: it is returned to the
/// caller as a reply.
///
/// # Examples
///
/// ```
/// use sc_agent_http::NetworkKind;
///
/// assert_eq!(NetworkKind::Timeout.as_str(), "timeout");
/// assert_eq!(NetworkKind::Connect.to_string(), "connect");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NetworkKind {
    /// The send timed out (connect or the whole-request deadline).
    /// Resent under `max_sends` and the start budget.
    Timeout,
    /// The connection could not be opened (DNS failure included).
    Connect,
    /// The TLS handshake failed.
    Tls,
    /// A transport error that is none of the above.
    Other,
}

impl NetworkKind {
    /// The snake_case spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::Connect => "connect",
            Self::Tls => "tls",
            Self::Other => "other",
        }
    }
}

impl fmt::Display for NetworkKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Recovery text of `http.provider_busy`.
const PROVIDER_BUSY_RECOVERY: &str = "the provider is busy; try again later or lower workers";
/// Recovery text of `http.quota_exhausted`.
const QUOTA_EXHAUSTED_RECOVERY: &str =
    "the provider account is out of quota or credit; add credit or use another key";
/// Recovery text of `http.key_on_plain_http`.
const KEY_ON_PLAIN_HTTP_RECOVERY: &str = "use an https endpoint or a loopback host (localhost, 127.0.0.0/8, ::1) for a runner that needs a key";

/// Every error a public sc-agent-http function returns.
///
/// `Display` writes `"<id>: <message>"` only. Provider text is redacted of
/// the request's key and otherwise kept whole (REQ-SAG-0002).
///
/// A [`Network`](Self::Network) or [`InvalidRequest`](Self::InvalidRequest)
/// made by [`UreqTransport`](crate::UreqTransport) carries the lower-layer
/// (ureq, io or http builder) error in `message` as
/// `"<foreign id>: <foreign message>"`, for example
/// `"io.ConnectionRefused: io: Connection refused (os error 61)"`;
/// [`ErrorId::cause_detail`] returns that pair as the cause. A message
/// without a foreign id (a test double's) has no cause.
///
/// # Examples
///
/// ```
/// use sc_agent_http::{HttpError, NetworkKind};
/// use sc_agent_types::ErrorId;
///
/// let error = HttpError::Network {
///     kind: NetworkKind::Connect,
///     attempts: 1,
///     message: "io.ConnectionRefused: connection refused".to_owned(),
/// };
/// assert_eq!(error.id(), "http.network");
/// assert_eq!(error.to_string(), format!("http.network: {}", error.message()));
/// let cause = error.detail().cause.expect("cause");
/// assert_eq!(cause.id, "io.ConnectionRefused");
/// assert_eq!(cause.message, "connection refused");
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum HttpError {
    /// Sends or budget ran out on busy replies (REQ-SAG-0601).
    ///
    /// `last` is the latest busy reply (always present: only a held busy
    /// request can end this way).
    ProviderBusy {
        /// Which limit refused the next send.
        reason: BusyReason,
        /// Sends made, the first included.
        attempts: u32,
        /// The last reply's valid `Retry-After`, as sent and unrounded
        /// (REQ-SAG-0007).
        retry_after_seconds: Option<f64>,
        /// The last busy reply.
        last: ProviderReply,
    },
    /// Busy status whose body marks quota, credit or spend exhaustion
    /// (REQ-SAG-0602). Returned after the send that got it, never resent.
    QuotaExhausted {
        /// Sends made, the first included.
        attempts: u32,
        /// What the provider said.
        reply: ProviderReply,
    },
    /// Transport failure.
    ///
    /// `Timeout`: resent under `max_sends` and the start budget, returned
    /// when either runs out. `Connect` (DNS included), `Tls`, `Other`:
    /// returned after one send.
    Network {
        /// The failure class.
        kind: NetworkKind,
        /// Sends made, the first included.
        attempts: u32,
        /// What failed, with the lower-layer text (see the type docs).
        message: String,
    },
    /// The body exceeded the 8 MiB read cap.
    ResponseTooLarge {
        /// The reply's HTTP status.
        status: u16,
        /// The cap, in bytes.
        limit_bytes: u64,
    },
    /// The URL does not parse, has no host, or its scheme is refused
    /// ([`Endpoint::parse`](crate::Endpoint::parse): https;
    /// [`Endpoint::parse_local`](crate::Endpoint::parse_local): http or https).
    InvalidEndpoint {
        /// The refused URL.
        url: String,
        /// Why it was refused.
        reason: String,
    },
    /// A header name or value the HTTP builder refuses.
    InvalidRequest {
        /// What the builder said (see the type docs).
        message: String,
    },
    /// A key was attached to a plain-http endpoint whose host is not
    /// loopback (REQ-SAG-0013). Carries the host only, never the key.
    KeyOnPlainHttp {
        /// The endpoint host.
        host: String,
    },
}

impl HttpError {
    /// This error with `map` applied to every text field (redaction).
    pub(crate) fn map_text(self, map: &impl Fn(&str) -> String) -> Self {
        match self {
            Self::ProviderBusy {
                reason,
                attempts,
                retry_after_seconds,
                last,
            } => Self::ProviderBusy {
                reason,
                attempts,
                retry_after_seconds,
                last: last.map_text(map),
            },
            Self::QuotaExhausted { attempts, reply } => Self::QuotaExhausted {
                attempts,
                reply: reply.map_text(map),
            },
            Self::Network {
                kind,
                attempts,
                message,
            } => Self::Network {
                kind,
                attempts,
                message: map(&message),
            },
            Self::ResponseTooLarge {
                status,
                limit_bytes,
            } => Self::ResponseTooLarge {
                status,
                limit_bytes,
            },
            Self::InvalidEndpoint { url, reason } => Self::InvalidEndpoint {
                url: map(&url),
                reason: map(&reason),
            },
            Self::InvalidRequest { message } => Self::InvalidRequest {
                message: map(&message),
            },
            Self::KeyOnPlainHttp { host } => Self::KeyOnPlainHttp { host: map(&host) },
        }
    }

    /// This error with its send count set to `sends` (a transport does not
    /// know how many sends the client made).
    pub(crate) fn with_attempts(self, sends: u32) -> Self {
        match self {
            Self::Network { kind, message, .. } => Self::Network {
                kind,
                attempts: sends,
                message,
            },
            other => other,
        }
    }
}

/// The lower-layer `(id, message)` pair at the front of a transport-made
/// message (`"<foreign id>: <foreign message>"`), if there is one.
///
/// A foreign id is one dotted token whose first segment is a known lower
/// layer (`ureq`, `io`, `http`), so free text is never mistaken for one.
fn foreign_cause(message: &str) -> Option<ErrorDetail> {
    let (id, text) = message.split_once(": ")?;
    let (layer, rest) = id.split_once('.')?;
    let known = matches!(layer, "ureq" | "io" | "http");
    let token = !rest.is_empty()
        && rest
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.'));
    (known && token).then(|| ErrorDetail::new(id, text, None))
}

impl ErrorId for HttpError {
    fn id(&self) -> &'static str {
        match self {
            Self::ProviderBusy { .. } => "http.provider_busy",
            Self::QuotaExhausted { .. } => "http.quota_exhausted",
            Self::Network { .. } => "http.network",
            Self::ResponseTooLarge { .. } => "http.response_too_large",
            Self::InvalidEndpoint { .. } => "http.invalid_endpoint",
            Self::InvalidRequest { .. } => "http.invalid_request",
            Self::KeyOnPlainHttp { .. } => "http.key_on_plain_http",
        }
    }

    fn message(&self) -> String {
        match self {
            Self::ProviderBusy {
                reason,
                attempts,
                retry_after_seconds,
                last,
            } => {
                let limit = match reason {
                    BusyReason::Sends => "no sends are left",
                    BusyReason::Budget => "the 90 s start budget is used up",
                };
                let retry_after = retry_after_seconds
                    .map(|seconds| format!(", Retry-After {seconds} s"))
                    .unwrap_or_default();
                format!(
                    "the provider was busy on {attempts} {} and {limit} ({reason}{retry_after}); last reply: {last}",
                    plural(*attempts)
                )
            }
            Self::QuotaExhausted { attempts, reply } => format!(
                "the provider account is out of quota or credit (after {attempts} {}): {reply}",
                plural(*attempts)
            ),
            Self::Network {
                kind,
                attempts,
                message,
            } => format!(
                "{kind} failure after {attempts} {}: {message}",
                plural(*attempts)
            ),
            Self::ResponseTooLarge {
                status,
                limit_bytes,
            } => format!("the HTTP {status} response body is over the {limit_bytes}-byte cap"),
            Self::InvalidEndpoint { url, reason } => {
                format!("endpoint {url:?} is invalid: {reason}")
            }
            Self::InvalidRequest { message } => format!("the request is invalid: {message}"),
            Self::KeyOnPlainHttp { host } => format!(
                "a key may not be sent over plain http to {host:?}, which is not a loopback host"
            ),
        }
    }

    fn cause_detail(&self) -> Option<ErrorDetail> {
        match self {
            Self::Network { message, .. } | Self::InvalidRequest { message } => {
                foreign_cause(message)
            }
            Self::ProviderBusy { .. }
            | Self::QuotaExhausted { .. }
            | Self::ResponseTooLarge { .. }
            | Self::InvalidEndpoint { .. }
            | Self::KeyOnPlainHttp { .. } => None,
        }
    }

    fn recovery(&self) -> Option<&'static str> {
        match self {
            Self::ProviderBusy { .. } => Some(PROVIDER_BUSY_RECOVERY),
            Self::QuotaExhausted { .. } => Some(QUOTA_EXHAUSTED_RECOVERY),
            Self::KeyOnPlainHttp { .. } => Some(KEY_ON_PLAIN_HTTP_RECOVERY),
            Self::Network { .. }
            | Self::ResponseTooLarge { .. }
            | Self::InvalidEndpoint { .. }
            | Self::InvalidRequest { .. } => None,
        }
    }
}

fn plural(sends: u32) -> &'static str {
    if sends == 1 { "send" } else { "sends" }
}

impl fmt::Display for HttpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.id(), self.message())
    }
}

impl std::error::Error for HttpError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foreign_cause_needs_a_known_dotted_id() {
        let cause = foreign_cause("ureq.timeout.Global: timeout: global").expect("cause");
        assert_eq!(
            (cause.id.as_str(), cause.message.as_str()),
            ("ureq.timeout.Global", "timeout: global")
        );
        for text in [
            "script exhausted",
            "request timed out: x",
            "ureq.: x",
            "net.x: y",
            "io.Conn refused: x",
        ] {
            assert_eq!(foreign_cause(text), None, "{text}");
        }
    }
}
