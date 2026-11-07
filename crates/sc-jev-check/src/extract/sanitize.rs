//! Child-process environment and stderr hygiene.
//!
//! Extractor and git children never inherit provider credentials. Their
//! stderr reaches [`JevError`] details only as a bounded tail with any
//! credential-looking environment value replaced by `[redacted]`.

use std::ffi::{OsStr, OsString};
use std::process::Command;

use sc_agent_types::JevError;

/// Bytes of child stderr kept for an error detail.
pub(crate) const STDERR_TAIL_BYTES: usize = 4096;

/// Shortest environment value treated as a secret when redacting.
///
/// Shorter values (`1`, `true`) would redact ordinary words.
const MIN_SECRET_LEN: usize = 8;

/// Variables an extractor child may inherit (`LC_*` is matched by prefix).
const ALLOWED_ENV: &[&str] = &[
    "PATH",
    "HOME",
    "USERPROFILE",
    "SYSTEMROOT",
    "TEMP",
    "TMP",
    "TMPDIR",
    "LANG",
    "TERM",
];

/// Clears `command`'s environment and copies only the allowlist from this process.
///
/// No `*_API_KEY` or `SC_*` variable can pass: neither matches an allowlist
/// entry or the `LC_` prefix.
pub(crate) fn apply_env_allowlist(command: &mut Command) {
    command.env_clear();
    for (name, value) in std::env::vars_os() {
        if is_allowed(&name) {
            command.env(name, value);
        }
    }
}

fn is_allowed(name: &OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    let upper = name.to_ascii_uppercase();
    upper.starts_with("LC_") || ALLOWED_ENV.contains(&upper.as_str())
}

fn is_secret_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    upper.ends_with("_API_KEY") || upper.starts_with("SC_")
}

/// Values of credential-looking variables in this process.
fn secret_values() -> Vec<OsString> {
    std::env::vars_os()
        .filter(|(name, value)| {
            name.to_str().is_some_and(is_secret_name) && value.len() >= MIN_SECRET_LEN
        })
        .map(|(_, value)| value)
        .collect()
}

/// Replaces every credential-looking environment value in `text`.
fn redact_text(text: &str, secrets: &[OsString]) -> String {
    let mut out = text.to_owned();
    for secret in secrets {
        if let Some(secret) = secret.to_str() {
            out = out.replace(secret, "[redacted]");
        }
    }
    out
}

/// Redacts credential-looking environment values from every text field of `error`.
pub(crate) fn redact_error(error: JevError) -> JevError {
    let secrets = secret_values();
    if secrets.is_empty() {
        return error;
    }
    error.map_detail(|text| redact_text(text, &secrets))
}

/// Last [`STDERR_TAIL_BYTES`] of `stderr` as trimmed, lossy UTF-8.
///
/// Redaction happens when the text is placed in an error, through
/// [`redact_error`].
pub(crate) fn stderr_tail(stderr: &[u8]) -> String {
    let start = stderr.len().saturating_sub(STDERR_TAIL_BYTES);
    String::from_utf8_lossy(&stderr[start..]).trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_admits_locale_and_path_only() {
        for name in ["PATH", "Path", "LC_ALL", "LC_CTYPE", "LANG", "TMPDIR"] {
            assert!(is_allowed(OsStr::new(name)), "{name}");
        }
        for name in [
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "TYPESAFE_API_KEY",
            "SC_LOG_ROOT",
            "SC_ANY_SECRET",
            "GIT_DIR",
        ] {
            assert!(!is_allowed(OsStr::new(name)), "{name}");
        }
    }

    #[test]
    fn redact_text_replaces_secret_values() {
        let secrets = vec![OsString::from("sk-secret-value-123")];
        assert_eq!(
            redact_text("failed with sk-secret-value-123 here", &secrets),
            "failed with [redacted] here"
        );
    }

    #[test]
    fn stderr_tail_keeps_the_end() {
        let mut stderr = vec![b'x'; STDERR_TAIL_BYTES * 2];
        stderr.extend_from_slice(b"final line\n");
        let tail = stderr_tail(&stderr);
        assert!(tail.ends_with("final line"));
        assert!(tail.len() <= STDERR_TAIL_BYTES);
    }
}
