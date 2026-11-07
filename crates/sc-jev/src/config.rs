//! Packaged TypeSafe endpoint, model, and retry limits.
//!
//! Later Jev endpoints extend this crate in place (ADR-SAG-0012). No other
//! crate sends Jev API traffic.

use std::time::Duration;

use sc_agent_types::JevError;
use serde::Deserialize;

/// TypeSafe check/judge client settings.
///
/// The packaged defaults live in `jev.toml` and are the vendor pins for the
/// API version (`/v1`) and the evaluator model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JevClientConfig {
    model: String,
    host: String,
    path: String,
    attempts: u32,
    timeout: Duration,
}

/// On-disk shape of `jev.toml`. Every field is required; unknown keys are
/// rejected. Range checks live in [`JevClientConfig::try_new`] only.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawJevToml {
    model: String,
    host: String,
    path: String,
    attempts: u32,
    timeout_seconds: u64,
}

impl JevClientConfig {
    /// Loads the committed TypeSafe pins.
    ///
    /// # Panics
    ///
    /// Panics if `jev.toml` is missing a field or fails validation. That file
    /// is compiled into the crate, so this is a packaging bug.
    #[must_use]
    pub fn packaged() -> Self {
        Self::from_toml(include_str!("jev.toml")).expect("packaged jev.toml is valid")
    }

    /// Parses a Jev client config document.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::ConfigInvalid`] when the text is not TOML, a
    /// required field is missing or has the wrong type, an unknown key is
    /// present, or a value fails [`Self::try_new`].
    pub fn from_toml(text: &str) -> Result<Self, JevError> {
        let raw: RawJevToml = toml::from_str(text)
            .map_err(|error| JevError::config_invalid(format!("jev.toml: {}", error.message())))?;
        Self::try_new(
            raw.model,
            raw.host,
            raw.path,
            raw.attempts,
            Duration::from_secs(raw.timeout_seconds),
        )
    }

    /// Builds a config from validated parts.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::ConfigInvalid`] when the model, host, path,
    /// attempt count, or timeout cannot be used for a check request.
    pub fn try_new(
        model: impl Into<String>,
        host: impl Into<String>,
        path: impl Into<String>,
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

fn invalid(detail: impl Into<String>) -> JevError {
    JevError::config_invalid(detail.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = "model = \"m\"\nhost = \"h.example\"\npath = \"/v1/x\"\nattempts = 2\ntimeout_seconds = 5\n";

    #[test]
    fn from_toml_reads_every_field() {
        let config = JevClientConfig::from_toml(VALID).expect("valid");
        assert_eq!(config.model(), "m");
        assert_eq!(config.endpoint(), "https://h.example/v1/x");
        assert_eq!(config.attempts(), 2);
        assert_eq!(config.timeout(), Duration::from_secs(5));
    }

    #[test]
    fn from_toml_rejects_bad_documents() {
        let cases = [
            ("not = [toml", "jev.toml"),
            (&VALID.replace("attempts = 2\n", "") as &str, "attempts"),
            (&format!("{VALID}extra = 1\n"), "extra"),
            (
                &VALID.replace("timeout_seconds = 5", "timeout_seconds = 0"),
                "timeout",
            ),
            (
                &VALID.replace("timeout_seconds = 5", "timeout_seconds = 3601"),
                "timeout",
            ),
            (
                &VALID.replace("timeout_seconds = 5", "timeout_seconds = -1"),
                "jev.toml",
            ),
            (&VALID.replace("attempts = 2", "attempts = 9"), "attempts"),
            (
                &format!("{VALID}max_request_bytes = 10\n"),
                "max_request_bytes",
            ),
            (&VALID.replace("\"m\"", "\"\""), "model"),
            (&VALID.replace("h.example", "https://h.example"), "host"),
            (&VALID.replace("/v1/x", "v1/x"), "path"),
        ];
        for (text, needle) in cases {
            let error = JevClientConfig::from_toml(text).expect_err(needle);
            assert_eq!(error.code(), "CONFIG_INVALID", "{needle}");
            assert!(error.to_string().contains(needle), "{needle}: {error}");
        }
    }
}
