/// Every failure is a value with a stable id; nothing panics or raises.
#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Config(String),
    #[error("no API key for {provider}: set {var}, add it to .env, or store it in the keyring")]
    MissingKey { provider: String, var: String },
    #[error("keyring override is ambiguous: ranked providers {0}")]
    AmbiguousKeyringOverride(String),
    #[error("keyring: {0}")]
    Keyring(String),
    #[error("could not build the request: {0}")]
    Encode(String),
    #[error("provider busy after {attempts} sends ({reason})")]
    ProviderBusy {
        attempts: u32,
        status: u16,
        retry_after_ms: Option<u64>,
        retry_after_seconds: Option<u64>,
        reason: BusyReason,
    },
    #[error("quota exhausted (status {status}): {body}")]
    QuotaExhausted {
        status: u16,
        body: String,
        attempts: u32,
        retry_after_ms: Option<u64>,
    },
    #[error("provider returned status {status}: {body}")]
    ApiStatus {
        status: u16,
        body: String,
        retry_after_ms: Option<u64>,
    },
    #[error("network: {message}")]
    Network { message: String, attempts: u32 },
    #[error("reply is not the expected JSON: {message}")]
    ResponseParse {
        message: String,
        status: Option<u16>,
    },
    /// A well-formed reply that does not answer the request; resent up to max sends.
    #[error("reply does not match the request: {0}")]
    Invalid(String),
    #[error("cancelled after {sends} sends")]
    Cancelled { sends: u32 },
    #[error("log: {0}")]
    Log(String),
}

/// Why a busy request stopped being retried.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BusyReason {
    #[error("sends")]
    Sends,
    #[error("budget")]
    Budget,
}

impl Error {
    pub fn id(&self) -> &'static str {
        match self {
            Self::Config(_) => "config.invalid",
            Self::MissingKey { .. } => "secrets.missing_key",
            Self::Keyring(_) => "secrets.keyring",
            Self::AmbiguousKeyringOverride(_) => "decidr.ambiguous_keyring_override",
            Self::Encode(_) => "request.encode",
            Self::ProviderBusy { .. } => "http.provider_busy",
            Self::QuotaExhausted { .. } => "http.quota_exhausted",
            Self::ApiStatus { .. } => "api.status",
            Self::Network { .. } => "http.network",
            Self::ResponseParse { .. } => "response.parse",
            Self::Invalid(_) => "response.schema_mismatch",
            Self::Cancelled { .. } => "request.cancelled",
            Self::Log(_) => "log.open",
        }
    }

    /// Busy, network and invalid replies are resent; nothing else is.
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::ProviderBusy { .. } | Self::Network { .. } | Self::Invalid(_)
        )
    }
}

/// The serializable native error contract, built only through ErrorId::detail.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ErrorDetail {
    pub id: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub attempts: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub retry_after_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub category: Option<String>,
}

pub trait ErrorId {
    fn detail(&self) -> ErrorDetail;
}

impl Error {
    pub fn category(&self) -> &'static str {
        match self {
            Self::Config(_) | Self::AmbiguousKeyringOverride(_) | Self::Keyring(_) => "config",
            Self::MissingKey { .. } => "auth",
            Self::ProviderBusy { .. } => "busy",
            Self::QuotaExhausted { .. } => "quota",
            Self::ApiStatus { status, .. } => match status {
                401 | 403 => "auth",
                402 => "quota",
                400 | 422 => "validation",
                429 | 500 | 503 | 504 => "busy",
                _ => "other",
            },
            Self::Network { .. } => "network",
            Self::ResponseParse { .. } | Self::Invalid(_) => "response_invalid",
            Self::Encode(_) => "validation",
            Self::Cancelled { .. } => "cancelled",
            Self::Log(_) => "log",
        }
    }
}

impl ErrorId for Error {
    fn detail(&self) -> ErrorDetail {
        let status = match self {
            Self::ProviderBusy { status, .. }
            | Self::QuotaExhausted { status, .. }
            | Self::ApiStatus { status, .. } => Some(*status),
            Self::ResponseParse { status, .. } => *status,
            _ => None,
        };
        let attempts = match self {
            Self::ProviderBusy { attempts, .. }
            | Self::QuotaExhausted { attempts, .. }
            | Self::Network { attempts, .. } => Some(*attempts),
            Self::Cancelled { sends } => Some(*sends),
            _ => None,
        };
        let retry_after_ms = match self {
            Self::ProviderBusy { retry_after_ms, .. }
            | Self::QuotaExhausted { retry_after_ms, .. }
            | Self::ApiStatus { retry_after_ms, .. } => *retry_after_ms,
            _ => None,
        };
        ErrorDetail {
            id: self.id().into(),
            message: self.to_string(),
            status,
            attempts,
            retry_after_ms,
            category: Some(self.category().into()),
        }
    }
}
