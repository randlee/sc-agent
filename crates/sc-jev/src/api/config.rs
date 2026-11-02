//! Packaged TypeSafe endpoint, model, and retry limits.

use std::time::Duration;

use sc_agent_types::JevError;

/// TypeSafe check/judge client settings.
///
/// The packaged defaults live in `api/jev.toml` and are the vendor pins for
/// the API version (`/v1`) and the evaluator model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JevClientConfig {
    model: String,
    host: String,
    path: String,
    max_request_bytes: u64,
    attempts: u32,
    timeout: Duration,
}

impl JevClientConfig {
    /// Loads the committed TypeSafe pins.
    ///
    /// # Panics
    ///
    /// Panics if `api/jev.toml` is missing a field or fails validation. That
    /// file is compiled into the crate, so this is a packaging bug.
    #[must_use]
    pub fn packaged() -> Self {
        Self::from_toml(include_str!("jev.toml")).expect("packaged api/jev.toml is valid")
    }

    /// Parses a Jev client config document.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::ConfigInvalid`] when a required field is missing
    /// or a value is outside the accepted range.
    pub fn from_toml(text: &str) -> Result<Self, JevError> {
        let table: toml::Table = text.parse().map_err(|error| JevError::ConfigInvalid {
            detail: format!("jev.toml: {error}"),
        })?;
        let timeout_seconds = required_u64(&table, "timeout_seconds")?;
        if !(1..=3_600).contains(&timeout_seconds) {
            return Err(JevError::ConfigInvalid {
                detail: "timeout_seconds must be from 1 to 3600".to_owned(),
            });
        }
        Self::try_new(
            required_string(&table, "model")?,
            required_string(&table, "host")?,
            required_string(&table, "path")?,
            required_u64(&table, "max_request_bytes")?,
            required_u32(&table, "attempts")?,
            Duration::from_secs(timeout_seconds),
        )
    }

    /// Builds a config from validated parts.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::ConfigInvalid`] when the model, host, path, byte
    /// cap, attempt count, or timeout cannot be used for a check request.
    pub fn try_new(
        model: impl Into<String>,
        host: impl Into<String>,
        path: impl Into<String>,
        max_request_bytes: u64,
        attempts: u32,
        timeout: Duration,
    ) -> Result<Self, JevError> {
        let model = model.into();
        let host = host.into();
        let path = path.into();
        if model.is_empty() || model.bytes().any(|byte| byte.is_ascii_control()) {
            return Err(invalid(
                "model must be a non-empty string without control characters",
            ));
        }
        if host.is_empty()
            || host.contains("://")
            || host.contains('/')
            || host.bytes().any(|byte| byte.is_ascii_whitespace())
        {
            return Err(invalid(
                "host must be a bare hostname such as api.typesafe.ai",
            ));
        }
        if !path.starts_with('/') || path.bytes().any(|byte| byte.is_ascii_whitespace()) {
            return Err(invalid(
                "path must be an absolute URL path such as /v1/systemone",
            ));
        }
        if max_request_bytes == 0 {
            return Err(invalid("max_request_bytes must be at least 1"));
        }
        if !(1..=8).contains(&attempts) {
            return Err(invalid("attempts must be from 1 to 8"));
        }
        if timeout.is_zero() || timeout > Duration::from_secs(3_600) {
            return Err(invalid("timeout must be from 1 second to 3600 seconds"));
        }
        Ok(Self {
            model,
            host,
            path,
            max_request_bytes,
            attempts,
            timeout,
        })
    }

    /// Evaluator model id sent on the wire.
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    /// API hostname, without a scheme.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Absolute URL path, including the `/v1` API version prefix.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Maximum check body size in bytes. One check is never split.
    #[must_use]
    pub fn max_request_bytes(&self) -> u64 {
        self.max_request_bytes
    }

    /// Total sends on a busy reply, including the first.
    #[must_use]
    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    /// Per-request timeout applied by the transport.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// `https://{host}{path}` for this config.
    #[must_use]
    pub fn endpoint(&self) -> String {
        format!("https://{}{}", self.host, self.path)
    }
}

fn required_string(table: &toml::Table, key: &str) -> Result<String, JevError> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("jev.toml is missing string `{key}`")))
}

fn required_u64(table: &toml::Table, key: &str) -> Result<u64, JevError> {
    let value = table
        .get(key)
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| invalid(format!("jev.toml is missing integer `{key}`")))?;
    u64::try_from(value)
        .map_err(|_| invalid(format!("jev.toml `{key}` must be a positive integer")))
}

fn required_u32(table: &toml::Table, key: &str) -> Result<u32, JevError> {
    let value = required_u64(table, key)?;
    u32::try_from(value).map_err(|_| invalid(format!("jev.toml `{key}` does not fit in u32")))
}

fn invalid(detail: impl Into<String>) -> JevError {
    JevError::ConfigInvalid {
        detail: detail.into(),
    }
}
