//! CLI stderr JSON shape (NFR-SAG-0001, ADR-SAG-0027 field rule).
//!
//! Success envelopes stay in the composition roots. This crate only defines
//! the error body so every CLI crate copies ids from [`crate::JevError`].

use serde::{Deserialize, Serialize};

use crate::error::JevError;

/// The only `schema_version` this crate writes or accepts.
pub(crate) const ERROR_RESPONSE_SCHEMA_VERSION: u32 = 1;

/// stderr body for a failed CLI command.
///
/// Serialized keys, in order: `error`, `message`, `schema_version`,
/// `retryable`, `details`.
///
/// - `schema_version` is always `1` from [`Self::from_jev_error`]. It
///   versions this error shape only, independent of the stdout envelope
///   `version`, and is bumped only by a breaking change of this shape.
/// - `retryable` is present (as `true`) only when [`JevError::retryable`] is
///   true; it is never written as `false`.
/// - `details` is present only for `ProducerError`, as `{"producer": <name>}`;
///   it is a closed object.
///
/// Reading (producer stderr) denies unknown keys at the top level and inside
/// `details`, and accepts `schema_version` absent (a Phase A producer) or
/// `1`; anything else does not parse. Adding any key is therefore a breaking
/// change and bumps `schema_version`. Consumers that see a `schema_version`
/// other than `1` treat the line as an unknown error shape and fall back to
/// the exit code and the raw line.
///
/// No field holds a secret (NFR-SAG-0002).
///
/// # Examples
///
/// ```
/// use sc_agent_types::{JevError, ScAgentErrorResponse};
///
/// let body = ScAgentErrorResponse::from_jev_error(&JevError::InputEmpty);
/// assert_eq!(
///     serde_json::to_string(&body).unwrap(),
///     r#"{"error":"INPUT_EMPTY","message":"Provide a non-empty input and retry.","schema_version":1}"#
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawErrorResponse")]
pub struct ScAgentErrorResponse {
    error: String,
    message: String,
    schema_version: u32,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<ErrorDetails>,
}

/// The closed `details` object of an error response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ErrorDetails {
    producer: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawErrorResponse {
    error: String,
    message: String,
    #[serde(default)]
    schema_version: Option<u32>,
    #[serde(default)]
    retryable: bool,
    #[serde(default)]
    details: Option<ErrorDetails>,
}

impl TryFrom<RawErrorResponse> for ScAgentErrorResponse {
    type Error = String;

    fn try_from(raw: RawErrorResponse) -> Result<Self, Self::Error> {
        match raw.schema_version {
            None | Some(ERROR_RESPONSE_SCHEMA_VERSION) => Ok(Self {
                error: raw.error,
                message: raw.message,
                schema_version: ERROR_RESPONSE_SCHEMA_VERSION,
                retryable: raw.retryable,
                details: raw.details,
            }),
            Some(other) => Err(format!(
                "error response schema_version {other} is not {ERROR_RESPONSE_SCHEMA_VERSION}"
            )),
        }
    }
}

impl ScAgentErrorResponse {
    /// Builds the response for `error` by the field rule above.
    ///
    /// This is the only Rust constructor. The message is whatever
    /// [`JevError::fix_hint`] returns.
    #[must_use]
    pub fn from_jev_error(error: &JevError) -> Self {
        let details = match error {
            JevError::ProducerError { producer, .. } => Some(ErrorDetails {
                producer: producer.clone(),
            }),
            _ => None,
        };
        Self {
            error: error.code().as_str().to_owned(),
            message: error.fix_hint(),
            schema_version: ERROR_RESPONSE_SCHEMA_VERSION,
            retryable: error.retryable(),
            details,
        }
    }

    /// Returns the stable error id.
    #[must_use]
    pub fn error(&self) -> &str {
        &self.error
    }

    /// Returns the fix hint copied from [`JevError::fix_hint`].
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the shape version (always `1`).
    #[must_use]
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Whether the error is retryable.
    #[must_use]
    pub fn retryable(&self) -> bool {
        self.retryable
    }

    /// The producer named in `details`, if any.
    #[must_use]
    pub fn producer(&self) -> Option<&str> {
        self.details
            .as_ref()
            .map(|details| details.producer.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_includes_safe_detail_and_no_secret_key() {
        let body = ScAgentErrorResponse::from_jev_error(&JevError::config_invalid("missing model"));
        assert!(body.message().contains("missing model"));
        let value = serde_json::to_value(&body).expect("serialize");
        for key in value.as_object().expect("object").keys() {
            let lower = key.to_ascii_lowercase();
            for forbidden in ["secret", "token", "password", "authorization", "credential"] {
                assert!(!lower.contains(forbidden), "{key}");
            }
        }
    }

    #[test]
    fn reader_rejects_unknown_keys_and_versions() {
        for line in [
            r#"{"error":"INPUT_EMPTY","message":"m","extra":1}"#,
            r#"{"error":"INPUT_EMPTY","message":"m","schema_version":2}"#,
            r#"{"error":"INPUT_EMPTY","message":"m","details":{"producer":"p","x":1}}"#,
        ] {
            assert!(
                serde_json::from_str::<ScAgentErrorResponse>(line).is_err(),
                "{line}"
            );
        }
        let phase_a: ScAgentErrorResponse =
            serde_json::from_str(r#"{"error":"INPUT_EMPTY","message":"m"}"#).expect("phase a");
        assert_eq!(phase_a.schema_version(), 1);
    }
}
