use std::fmt;
use std::sync::Arc;

use crate::error::JevError;
use crate::ids::BackendKeyId;

/// Recovery text for a key that fails [`ResolvedApiKey::try_new`].
const MALFORMED_KEY_RECOVERY: &str = "Store the key again in the process environment, the repo-root .env file, or the keyring without spaces, line breaks, control characters, or non-ASCII characters, then retry.";

/// API key accepted by a sending transport.
///
/// The secret is held as `Arc<str>`, so clones share one allocation.
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
pub struct ResolvedApiKey(Arc<str>);

impl ResolvedApiKey {
    /// Normalizes and seals a raw credential for backend `id`.
    ///
    /// This is the one rule set for every backend and the only constructor.
    /// Surrounding whitespace is trimmed. The trimmed key must be non-empty
    /// and must not contain whitespace or control characters, because no
    /// provider issues keys with them and a stray space or newline is a
    /// copy-paste error.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::KeyMissing`] when the trimmed key is empty.
    /// Returns [`JevError::CredentialStoreInvalid`] (`CONFIG_INVALID`) when
    /// the key contains interior whitespace or a control character. Neither
    /// error contains key bytes.
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
            return Err(JevError::key_missing(id));
        }
        if trimmed
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control())
        {
            return Err(JevError::credential_store_invalid(
                format!("the {id} API key"),
                "key contains whitespace or a control character",
                MALFORMED_KEY_RECOVERY,
            ));
        }
        // The key travels as an HTTP header value, which `http` accepts for
        // any byte but controls (so it takes a non-ASCII character).
        // Providers issue visible-ASCII keys only, so this is the one rule: a
        // key outside visible ASCII is refused here, and no later layer has a
        // failing header value to handle.
        if !trimmed.chars().all(|ch| ch.is_ascii_graphic()) {
            return Err(JevError::credential_store_invalid(
                format!("the {id} API key"),
                "key contains a character outside visible ASCII",
                MALFORMED_KEY_RECOVERY,
            ));
        }
        Ok(Self(Arc::from(trimmed)))
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

    /// Replaces the secret with `[redacted]` in `text`.
    ///
    /// Use it on text that is about to be cut to a length bound: redacting
    /// after the cut could leave the front half of a key that straddles the
    /// cut.
    #[must_use]
    pub fn redact_text(&self, text: &str) -> String {
        text.replace(&*self.0, "[redacted]")
    }

    /// Replaces the secret with `[redacted]` in every text field of `error`.
    ///
    /// Built on [`JevError::map_detail`], so a new variant with a text field
    /// is redacted without changing this method.
    #[must_use]
    pub fn redact_error(&self, error: JevError) -> JevError {
        error.map_detail(|detail| self.redact_text(detail))
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

    fn key(text: &str) -> ResolvedApiKey {
        ResolvedApiKey::try_new(BackendKeyId::TypeSafe, text.to_owned()).expect("key")
    }

    #[test]
    fn try_new_trims_and_keeps_the_key() {
        assert_eq!(key(&format!("  {SECRET}\r\n")), key(SECRET));
        let exact = ResolvedApiKey::try_new(BackendKeyId::OpenAi, SECRET.to_owned()).unwrap();
        assert_eq!(exact, key(SECRET));
    }

    #[test]
    fn try_new_has_one_rule_set_for_every_backend() {
        for id in BackendKeyId::ALL.iter().copied() {
            for blank in ["", "   ", "\n\t"] {
                let error = ResolvedApiKey::try_new(id, blank.to_owned()).unwrap_err();
                assert_eq!(error, JevError::key_missing(id));
            }
            for bad in [
                format!("{SECRET} tail"),
                format!("{SECRET}\ttail"),
                format!("{SECRET}\u{7}"),
                format!("a\u{0}{SECRET}"),
                format!("{SECRET}\u{a0}x"),
                format!("{SECRET}\u{7f}"),
            ] {
                let error = ResolvedApiKey::try_new(id, bad.clone()).unwrap_err();
                assert_eq!(error.code(), "CONFIG_INVALID", "{id} {bad:?}");
                let hint = error.message();
                assert!(hint.contains(&id.to_string()), "{hint}");
                assert!(hint.contains("whitespace or a control character"), "{hint}");
                assert!(!hint.contains("check.toml"), "{hint}");
                let rendered = format!("{error} {error:?} {hint}");
                assert!(!rendered.contains(SECRET), "{rendered}");
            }
        }
    }

    /// The one header-value rule is here: a key outside visible ASCII is
    /// refused, with the credential recovery text and no key bytes.
    #[test]
    fn try_new_refuses_non_ascii_keys() {
        for id in BackendKeyId::ALL.iter().copied() {
            for bad in [format!("{SECRET}é"), format!("日本{SECRET}")] {
                let error = ResolvedApiKey::try_new(id, bad.clone()).unwrap_err();
                assert_eq!(error.code(), "CONFIG_INVALID", "{id} {bad:?}");
                assert!(error.message().contains("outside visible ASCII"));
                assert!(error.suggested_action().contains("non-ASCII"));
                let rendered = format!("{error} {error:?} {}", error.suggested_action());
                assert!(!rendered.contains(SECRET), "{rendered}");
            }
        }
    }
