//! Sealed API key (ADR-SAG-0020).
//!
//! [`ResolvedApiKey::as_str`] is the only way to read the secret bytes. Only
//! the transport that sends the key may call it; `clippy.toml` lists it under
//! `disallowed-methods` so every other read needs an explicit, reviewed
//! `#[allow]`. `sc-agent-secrets` resolves the key through
//! [`ResolvedApiKey::try_new`]; Jev and LLM backends accept this type and do
//! not depend on that crate.

use std::fmt;

use crate::error::JevError;
use crate::ids::BackendKeyId;

/// Recovery text for a key that fails [`ResolvedApiKey::try_new`].
const MALFORMED_KEY_RECOVERY: &str = "Store the key again in the process environment, the repo-root .env file, or the keyring without spaces, line breaks, or other control characters, then retry.";

/// API key accepted by a sending transport.
///
/// [`Debug`](fmt::Debug) and [`Display`](fmt::Display) print `[redacted]` and
/// never the secret bytes.
///
/// # Examples
///
/// ```
/// use sc_agent_types::{BackendKeyId, ResolvedApiKey};
///
/// let key = ResolvedApiKey::try_new(BackendKeyId::OpenAi, " test-key\n".to_owned())
///     .expect("well-formed key");
/// assert_eq!(format!("{key:?}"), "ResolvedApiKey([redacted])");
/// assert_eq!(key.to_string(), "[redacted]");
/// ```
#[must_use = "read the secret with as_str only at the transport that sends it"]
#[derive(Clone, PartialEq, Eq)]
pub struct ResolvedApiKey(String);

impl ResolvedApiKey {
    /// Normalizes and seals a raw credential for backend `id`.
    ///
    /// This is the one rule set for every backend. Surrounding whitespace is
    /// trimmed. The trimmed key must be non-empty and must not contain
    /// whitespace or control characters, because no provider issues keys
    /// with them and a stray space or newline is a copy-paste error.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::KeyMissing`] when the trimmed key is empty.
    /// Returns [`JevError::ConfigInvalid`] built by
    /// [`JevError::credential_store_invalid`] when the key contains interior
    /// whitespace or a control character. Neither error contains key bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::{BackendKeyId, ResolvedApiKey};
    ///
    /// let missing = ResolvedApiKey::try_new(BackendKeyId::TypeSafe, "  ".to_owned());
    /// assert_eq!(missing.unwrap_err().code(), "KEY_MISSING");
    /// let spaced = ResolvedApiKey::try_new(BackendKeyId::Anthropic, "sk ant".to_owned());
    /// assert_eq!(spaced.unwrap_err().code(), "CONFIG_INVALID");
    /// ```
    pub fn try_new(id: BackendKeyId, raw: String) -> Result<Self, JevError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(JevError::KeyMissing { backend: id });
        }
        if trimmed
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control())
        {
            return Err(JevError::credential_store_invalid(
                &format!("the {id} API key"),
                "key contains whitespace or a control character",
                MALFORMED_KEY_RECOVERY,
            ));
        }
        if trimmed.len() == raw.len() {
            Ok(Self(raw))
        } else {
            Ok(Self(trimmed.to_owned()))
        }
    }

    /// Seals `secret` without any check. Test-only.
    ///
    /// Production code builds keys with [`Self::try_new`] (through
    /// `sc-agent-secrets`). This constructor exists so tests can build a
    /// fixed key; it is hidden from the docs and is not part of the supported
    /// API.
    #[doc(hidden)]
    pub fn new(secret: String) -> Self {
        Self(secret)
    }

    /// Returns the secret.
    ///
    /// This is the only read of the secret bytes. Only the transport that
    /// sends the key may call it. Do not log the result, store it in an
    /// error, or place it in [`Debug`](fmt::Debug) output.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Replaces the secret with `[redacted]` in every text field of `error`.
    ///
    /// Built on [`JevError::map_detail`], so a new variant with a text field
    /// is redacted without changing this method.
    #[must_use]
    pub fn redact_error(&self, error: JevError) -> JevError {
        if self.0.is_empty() {
            return error;
        }
        error.map_detail(|detail| detail.replace(&self.0, "[redacted]"))
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

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "sk-test-secret";

    #[test]
    fn try_new_trims_and_keeps_the_key() {
        let key = ResolvedApiKey::try_new(BackendKeyId::TypeSafe, format!("  {SECRET}\r\n"))
            .expect("trimmed");
        assert_eq!(key, ResolvedApiKey::new(SECRET.to_owned()));
        let exact = ResolvedApiKey::try_new(BackendKeyId::OpenAi, SECRET.to_owned()).unwrap();
        assert_eq!(exact, ResolvedApiKey::new(SECRET.to_owned()));
    }

    #[test]
    fn try_new_has_one_rule_set_for_every_backend() {
        for id in [
            BackendKeyId::TypeSafe,
            BackendKeyId::Anthropic,
            BackendKeyId::OpenAi,
        ] {
            for blank in ["", "   ", "\n\t"] {
                let error = ResolvedApiKey::try_new(id, blank.to_owned()).unwrap_err();
                assert_eq!(error, JevError::KeyMissing { backend: id });
            }
            for bad in [
                format!("{SECRET} tail"),
                format!("{SECRET}\ttail"),
                format!("{SECRET}\u{7}"),
                format!("a\u{0}{SECRET}"),
                format!("{SECRET}\u{a0}x"),
            ] {
                let error = ResolvedApiKey::try_new(id, bad.clone()).unwrap_err();
                assert_eq!(error.code(), "CONFIG_INVALID", "{id} {bad:?}");
                let hint = error.fix_hint();
                assert!(hint.contains(&id.to_string()), "{hint}");
                assert!(hint.contains("whitespace or a control character"), "{hint}");
                assert!(!hint.contains("check.toml"), "{hint}");
                let rendered = format!("{error} {error:?} {hint}");
                assert!(!rendered.contains(SECRET), "{rendered}");
            }
        }
    }

    #[test]
    fn redact_error_covers_every_text_field() {
        let key = ResolvedApiKey::new(SECRET.to_owned());
        let error = JevError::ExtractionParseError {
            path: format!("dir/{SECRET}.py"),
            line: 1,
        };
        let redacted = key.redact_error(error);
        assert_eq!(
            redacted,
            JevError::ExtractionParseError {
                path: "dir/[redacted].py".to_owned(),
                line: 1,
            }
        );
        let network = key.redact_error(JevError::NetworkError {
            detail: format!("Bearer {SECRET} rejected"),
        });
        assert!(!network.to_string().contains(SECRET));
        let empty = ResolvedApiKey::new(String::new());
        let untouched = JevError::InputInvalid {
            detail: "abc".to_owned(),
        };
        assert_eq!(empty.redact_error(untouched.clone()), untouched);
    }
}
