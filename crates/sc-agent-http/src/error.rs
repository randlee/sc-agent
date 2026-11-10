/// Transport failure class of a [`JevError::NetworkError`].
///
/// A transport failure is one where no usable HTTP reply arrived. A reply
/// with a failing status is never a `NetworkErrorKind`: it is `AUTH_FAILED`,
/// `API_REQUEST_INVALID`, `QUOTA_EXHAUSTED`, `PROVIDER_BUSY`, `API_ERROR` or
/// `INPUT_TOO_LARGE`. The one spelling is snake_case for serde, `Display`,
/// and `FromStr`; it is the `network_kind` key of the stderr `details`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum NetworkErrorKind {
    /// The request timed out.
    Timeout,
    /// The connection could not be opened.
    Connect,
    /// The TLS handshake failed.
    Tls,
    /// The caller cancelled the request.
    Cancelled,
    /// A transport error that is none of the above. Not retryable: an
    /// unclassified failure is not assumed to be transient.
    Other,
}

impl NetworkErrorKind {
    /// Every kind, in declaration order.
    pub const ALL: &'static [Self] = &[
        Self::Timeout,
        Self::Connect,
        Self::Tls,
        Self::Cancelled,
        Self::Other,
    ];

    /// The snake_case spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::Connect => "connect",
            Self::Tls => "tls",
            Self::Cancelled => "cancelled",
            Self::Other => "other",
        }
    }

    /// Whether a later retry may succeed (`Timeout`, `Connect`).
    #[must_use]
    pub const fn retryable(self) -> bool {
        matches!(self, Self::Timeout | Self::Connect)
    }
}

impl fmt::Display for NetworkErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
/// Why a busy provider gave up after the busy retry (`PROVIDER_BUSY`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BusyCause {
    /// The last allowed send was busy.
    Sends,
    /// The retry budget left no time for another attempt.
    Budget,
}

impl BusyCause {
    /// The snake_case spelling (`sends`, `budget`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sends => "sends",
            Self::Budget => "budget",
        }
    }
}
