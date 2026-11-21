#![forbid(unsafe_code)]
//! API keys: process environment, then the repo-root `.env`, then the platform keyring.

use std::path::Path;

use sc_agent_types::Error;

/// Where a key was found (shown by doctor; the key itself never is).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeySource {
    Env,
    DotEnv,
    Keyring,
}

impl KeySource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Env => "environment",
            Self::DotEnv => ".env",
            Self::Keyring => "keyring",
        }
    }
}

/// The key for `var` from the real sources. `keyring` overrides the default service
/// (`var`) and account (the OS user).
pub fn key(
    var: &str,
    repo_root: Option<&Path>,
    keyring: Option<(&str, &str)>,
) -> Result<Option<(String, KeySource)>, Error> {
    lookup(
        var,
        |v| std::env::var(v).ok(),
        repo_root,
        |var| {
            let user = whoami::username();
            let (service, account) = keyring.unwrap_or((var, user.as_str()));
            let entry =
                keyring::Entry::new(service, account).map_err(|e| Error::Keyring(e.to_string()))?;
            match entry.get_password() {
                Ok(value) => Ok(Some(value)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(e) => Err(Error::Keyring(e.to_string())),
            }
        },
    )
}

/// The lookup order over given sources; a blank value counts as absent.
pub fn lookup(
    var: &str,
    env: impl Fn(&str) -> Option<String>,
    repo_root: Option<&Path>,
    keyring: impl Fn(&str) -> Result<Option<String>, Error>,
) -> Result<Option<(String, KeySource)>, Error> {
    let present =
        |value: Option<String>| value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
    if let Some(value) = present(env(var)) {
        return Ok(Some((value, KeySource::Env)));
    }
    if let Some(root) = repo_root
        && let Ok(entries) = dotenvy::from_path_iter(root.join(".env"))
    {
        let found = entries.filter_map(Result::ok).find(|(name, _)| name == var);
        if let Some(value) = present(found.map(|(_, value)| value)) {
            return Ok(Some((value, KeySource::DotEnv)));
        }
    }
    Ok(present(keyring(var)?).map(|value| (value, KeySource::Keyring)))
}

/// `text` with `key` replaced, so an error or log line never carries the key.
pub fn redact(text: &str, key: Option<&str>) -> String {
    match key {
        Some(key) if !key.is_empty() => text.replace(key, "[REDACTED]"),
        _ => text.to_owned(),
    }
}
