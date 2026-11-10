//! The sealed API key type.

use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

use crate::error::SecretsError;

/// An API key accepted by a sending transport.
///
/// The secret is held as `Arc<str>`, so clones share one allocation.
/// [`Debug`](fmt::Debug) and [`Display`](fmt::Display) print `[redacted]` and
/// never the secret bytes. The only way to build one is
/// [`resolve`](crate::resolve) or [`resolve_var`](crate::resolve_var).
///
/// # Examples
///
/// ```
/// use sc_agent_secrets::{resolve_var, EnvSource, KeySources, KeyringSource, SecretsError};
/// use sc_agent_types::VarName;
///
/// struct OneVar;
/// impl EnvSource for OneVar {
///     fn var(&self, name: &str) -> Option<String> {
///         (name == "MY_RUNNER_TOKEN").then(|| " test-key\n".to_owned())
///     }
/// }
/// struct NoKeyring;
/// impl KeyringSource for NoKeyring {
///     fn password(&self, _: &str, _: &str) -> Result<Option<String>, SecretsError> {
///         Ok(None)
///     }
/// }
///
/// let var: VarName = "MY_RUNNER_TOKEN".parse()?;
/// let sources = KeySources { env: &OneVar, repo_root: None, keyring: &NoKeyring };
/// let key = resolve_var(&var, &sources)?.key;
/// assert_eq!(format!("{key:?}"), "ApiKey([redacted])");
/// assert_eq!(key.to_string(), "[redacted]");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use = "read the secret with as_str only at the transport that sends it"]
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(Arc<str>);

impl ApiKey {
    /// Normalizes and seals a raw credential read from variable `var`.
    ///
    /// This is the one rule set for every key and the only constructor.
    /// Surrounding whitespace is trimmed. The trimmed key must be non-empty
    /// and consist of visible ASCII only, because no provider issues keys
    /// with whitespace or control characters and a stray space or newline is
    /// a copy-paste error.
    ///
    /// # Errors
    ///
    /// Returns [`SecretsError::KeyMissing`] when the trimmed key is empty and
    /// [`SecretsError::KeyMalformed`] when it contains whitespace, a control
    /// character or a character outside visible ASCII. Neither error contains
    /// key bytes.
    pub(crate) fn try_new(var: &str, raw: &str) -> Result<Self, SecretsError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(SecretsError::KeyMissing {
                var: var.to_owned(),
            });
        }
        // One rule, one scan: the key travels as an HTTP header value, and
        // providers issue visible-ASCII keys only, so anything else is refused
        // here and no later layer has a failing header value to handle. Only
        // the error path looks again, to name the likelier mistake: a stray
        // space, newline or control character (copy-paste) before a
        // non-ASCII character.
        if !trimmed.bytes().all(|byte| byte.is_ascii_graphic()) {
            let reason = if trimmed
                .chars()
                .any(|ch| ch.is_whitespace() || ch.is_control())
            {
                "key contains whitespace or a control character"
            } else {
                "key contains a character outside visible ASCII"
            };
            return Err(SecretsError::KeyMalformed {
                var: var.to_owned(),
                reason,
            });
        }
        Ok(Self(Arc::from(trimmed)))
    }

    /// Returns the secret.
    ///
    /// This is the only read of the secret bytes. Only the transport that
    /// sends the key may call it (the workspace `clippy.toml` disallows every
    /// other call). Do not log the result, store it in an error, or place it
    /// in [`Debug`](fmt::Debug) output.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Replaces the secret with `[redacted]` in `text`.
    ///
    /// Use it on text that is about to be cut to a length bound: redacting
    /// after the cut could leave the front half of a key that straddles the
    /// cut.
    ///
    /// Borrows `text` when it does not contain the secret.
    #[must_use]
    pub fn redact_text<'t>(&self, text: &'t str) -> Cow<'t, str> {
        if text.contains(&*self.0) {
            Cow::Owned(text.replace(&*self.0, "[redacted]"))
        } else {
            Cow::Borrowed(text)
        }
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApiKey([redacted])")
    }
}

impl fmt::Display for ApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[redacted]")
    }
}

#[cfg(test)]
mod tests {
    use sc_agent_types::{ErrorId, Provider};

    use super::*;
    use crate::{ALL_VARS, env_var};

    const SECRET: &str = "sk-test-secret";

    fn key(text: &str) -> ApiKey {
        ApiKey::try_new(env_var(Provider::TypeSafe), text).expect("key")
    }

    #[test]
    fn try_new_trims_and_keeps_the_key() {
        assert_eq!(key(&format!("  {SECRET}\r\n")), key(SECRET));
        let exact = ApiKey::try_new(env_var(Provider::OpenAi), SECRET).unwrap();
        assert_eq!(exact, key(SECRET));
    }

    #[test]
    #[expect(clippy::disallowed_methods, reason = "test reads the key it built")]
    fn as_str_returns_the_trimmed_secret() {
        assert_eq!(key(&format!(" {SECRET}\n")).as_str(), SECRET);
    }

    #[test]
    fn try_new_has_one_rule_set_for_every_variable() {
        let vars = Provider::ALL
            .iter()
            .map(|provider| env_var(*provider))
            .chain(ALL_VARS.iter().copied());
        for var in vars {
            for blank in ["", "   ", "\n\t"] {
                let error = ApiKey::try_new(var, blank).unwrap_err();
                assert_eq!(
                    error,
                    SecretsError::KeyMissing {
                        var: var.to_owned()
                    }
                );
            }
            for bad in [
                format!("{SECRET} tail"),
                format!("{SECRET}\ttail"),
                format!("{SECRET}\u{7}"),
                format!("a\u{0}{SECRET}"),
                format!("{SECRET}\u{a0}x"),
                format!("{SECRET}\u{7f}"),
            ] {
                let error = ApiKey::try_new(var, &bad).unwrap_err();
                assert_eq!(error.id(), "secrets.key_malformed", "{var} {bad:?}");
                let hint = error.message();
                assert!(hint.contains(var), "{hint}");
                assert!(hint.contains("whitespace or a control character"), "{hint}");
                let rendered = format!("{error} {error:?} {hint}");
                assert!(!rendered.contains(SECRET), "{rendered}");
            }
        }
    }

    /// The one header-value rule is here: a key outside visible ASCII is
    /// refused, with recovery text and no key bytes.
    #[test]
    fn try_new_refuses_non_ascii_keys() {
        for provider in Provider::ALL.iter().copied() {
            let var = env_var(provider);
            for bad in [format!("{SECRET}é"), format!("日本{SECRET}")] {
                let error = ApiKey::try_new(var, &bad).unwrap_err();
                assert_eq!(error.id(), "secrets.key_malformed", "{var} {bad:?}");
                assert!(error.message().contains("outside visible ASCII"));
                let recovery = error.recovery().unwrap_or_default();
                assert!(recovery.contains("visible ASCII"), "{recovery}");
                let rendered = format!("{error} {error:?} {recovery}");
                assert!(!rendered.contains(SECRET), "{rendered}");
            }
        }
    }

    #[test]
    fn debug_display_and_redact_text_hide_the_secret() {
        let key = key(SECRET);
        assert_eq!(format!("{key:?}"), "ApiKey([redacted])");
        assert_eq!(key.to_string(), "[redacted]");
        assert_eq!(
            key.redact_text(&format!("auth: Bearer {SECRET}; again {SECRET}")),
            "auth: Bearer [redacted]; again [redacted]"
        );
        let clean = "no secret here";
        assert!(matches!(key.redact_text(clean), Cow::Borrowed(text) if text == clean));
    }
}
