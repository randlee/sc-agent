//! Sealed API key (ADR-SAG-0020).
//!
//! [`ResolvedApiKey::as_str`] is the only way to read the secret bytes. Only
//! the transport that sends the key may call it. `sc-agent-secrets` resolves
//! the key; Jev and LLM backends accept this type and do not depend on that
//! crate.

use std::fmt;

use crate::error::JevError;
use crate::ids::BackendKeyId;

/// API key accepted by a sending transport.
///
/// [`Debug`] and [`Display`] print `[redacted]` and never the secret bytes.
///
/// # Examples
///
/// ```
/// use sc_agent_types::ResolvedApiKey;
///
/// let key = ResolvedApiKey::new("test-key".to_owned());
/// assert_eq!(format!("{key:?}"), "ResolvedApiKey([redacted])");
/// assert_eq!(key.to_string(), "[redacted]");
/// ```
#[must_use = "read the secret with as_str only at the transport that sends it"]
#[derive(Clone, PartialEq, Eq)]
pub struct ResolvedApiKey(String);

impl ResolvedApiKey {
    /// Seals `secret` for a sending transport.
    ///
    /// Secret resolution calls this after it has trimmed the key. Other
    /// callers that still hold a raw string use [`Self::normalized_typesafe`]
    /// or [`Self::normalized_provider`] so the backend constructor does not
    /// take the string itself.
    pub fn new(secret: String) -> Self {
        Self(secret)
    }

    /// Returns the secret.
    ///
    /// This is the only read of the secret bytes. Only the transport that
    /// sends the key may call it. Do not log the result, store it in an
    /// error, or place it in [`Debug`] output.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Checks a TypeSafe key and stores the trimmed secret.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::KeyMissing`] when the trimmed key is empty, and
    /// [`JevError::ConfigInvalid`] when it contains whitespace or a control
    /// character.
    pub fn normalized_typesafe(self) -> Result<Self, JevError> {
        let key = self.0.trim();
        if key.is_empty() {
            return Err(JevError::KeyMissing {
                backend: BackendKeyId::TypeSafe,
            });
        }
        if key
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
        {
            return Err(JevError::ConfigInvalid {
                detail: "TypeSafe API key contains whitespace or a control character".to_owned(),
            });
        }
        Ok(Self(key.to_owned()))
    }

    /// Checks a provider key and stores the trimmed secret.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::KeyMissing`] when the trimmed key is empty or
    /// contains a control character.
    pub fn normalized_provider(self, backend: BackendKeyId) -> Result<Self, JevError> {
        let key = self.0.trim();
        if key.is_empty() || key.chars().any(char::is_control) {
            return Err(JevError::KeyMissing { backend });
        }
        Ok(Self(key.to_owned()))
    }

    /// Replaces the secret in a [`JevError`] detail with `[redacted]`.
    #[must_use]
    pub fn redact_error(&self, error: JevError) -> JevError {
        let redact = |detail: String| -> String {
            if self.0.is_empty() {
                return detail;
            }
            detail.replace(&self.0, "[redacted]")
        };
        match error {
            JevError::NetworkError { detail } => JevError::NetworkError {
                detail: redact(detail),
            },
            JevError::ResponseInvalid { detail } => JevError::ResponseInvalid {
                detail: redact(detail),
            },
            JevError::InputInvalid { detail } => JevError::InputInvalid {
                detail: redact(detail),
            },
            JevError::ConfigInvalid { detail } => JevError::ConfigInvalid {
                detail: redact(detail),
            },
            other => other,
        }
    }
}

impl fmt::Debug for ResolvedApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ResolvedApiKey([redacted])")
    }
}

impl fmt::Display for ResolvedApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[redacted]")
    }
}
