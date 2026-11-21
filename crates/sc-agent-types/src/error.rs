/// Every failure is a value with a stable id; nothing panics or raises.
#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("timeout_secs {0} must be a finite number in 10..=3600")]
    InvalidRequestTimeout(f64),
    #[error("workers {0} must be an integer in 1..=64 (batch only)")]
    InvalidWorkers(i64),
    #[error("{0}")]
    Config(String),
    #[error("no API key for {provider}: set {var}, add it to .env, or store it in the keyring")]
    MissingKey { provider: String, var: String },
    #[error("keyring: {0}")]
    Keyring(String),
    #[error("could not build the request: {0}")]
    Encode(String),
    #[error("provider busy after {attempts} sends")]
    ProviderBusy {
        attempts: u32,
        retry_after_seconds: Option<u64>,
    },
    #[error("quota exhausted (status {status}): {body}")]
    QuotaExhausted { status: u16, body: String },
    #[error("provider returned status {status}: {body}")]
    ApiStatus { status: u16, body: String },
    #[error("network: {0}")]
    Network(String),
    #[error("reply is not the expected JSON: {0}")]
    ResponseParse(String),
    /// A well-formed reply that does not answer the request; resent up to max sends.
    #[error("reply does not match the request: {0}")]
    Invalid(String),
    #[error("cancelled after {sends} sends")]
    Cancelled { sends: u32 },
    #[error("log: {0}")]
    Log(String),
}

impl Error {
    pub fn id(&self) -> &'static str {
        match self {
            Self::InvalidRequestTimeout(_) => "decidr.invalid_request_timeout",
            Self::InvalidWorkers(_) => "decidr.invalid_workers",
            Self::Config(_) => "config.invalid",
            Self::MissingKey { .. } => "secrets.missing_key",
            Self::Keyring(_) => "secrets.keyring",
            Self::Encode(_) => "request.encode",
            Self::ProviderBusy { .. } => "http.provider_busy",
            Self::QuotaExhausted { .. } => "http.quota_exhausted",
            Self::ApiStatus { .. } => "api.status",
            Self::Network(_) => "http.network",
            Self::ResponseParse(_) => "response.parse",
            Self::Invalid(_) => "response.schema_mismatch",
            Self::Cancelled { .. } => "request.cancelled",
            Self::Log(_) => "log.open",
        }
    }

    /// Busy, network and invalid replies are resent; nothing else is.
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::ProviderBusy { .. } | Self::Network(_) | Self::Invalid(_)
        )
    }
}
