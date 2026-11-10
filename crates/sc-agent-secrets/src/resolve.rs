//! Key resolution: process environment, then repo-root `.env`, then keyring.

use std::path::Path;

use sc_agent_types::{Provider, VarName};

use crate::api_key::ApiKey;
use crate::dotenv::load_dotenv;
use crate::env::{EnvSource, ProcessEnv};
use crate::error::SecretsError;
use crate::keyring::{KeyringSource, PlatformKeyring, current_account};
use crate::vars::env_var;

/// Where keys are looked up, in this order (REQ-SAG-0401).
///
/// The process environment first, then `<repo_root>/.env`, then the keyring.
/// Tests build one from their own [`EnvSource`] and [`KeyringSource`], so no
/// test reads or writes the process environment or the real keyring.
pub struct KeySources<'a> {
    /// The environment read first.
    pub env: &'a dyn EnvSource,
    /// The git work-tree top level whose `.env` is read second; `None` reads no `.env`.
    pub repo_root: Option<&'a Path>,
    /// The keyring read last (service = variable name, account = OS user).
    pub keyring: &'a dyn KeyringSource,
}

impl<'a> KeySources<'a> {
    /// The process environment and the platform keyring.
    ///
    /// `repo_root` is the git work-tree top level
    /// (`sc_agent_log::Repo::work_tree_root`), or `None` outside git, in which
    /// case no `.env` is read.
    #[must_use]
    pub fn system(repo_root: Option<&'a Path>) -> Self {
        Self {
            env: &ProcessEnv,
            repo_root,
            keyring: &PlatformKeyring,
        }
    }
}

/// Which source supplied a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource {
    /// The process environment (or the caller's [`EnvSource`]).
    Env,
    /// The repo-root `.env` file.
    DotEnv,
    /// The platform keyring (or the caller's [`KeyringSource`]).
    Keyring,
}

impl KeySource {
    /// `"env"`, `".env"` or `"keyring"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Env => "env",
            Self::DotEnv => ".env",
            Self::Keyring => "keyring",
        }
    }
}

/// A key and the source that supplied it (`decide doctor` lists the source).
#[derive(Debug, Clone)]
pub struct ResolvedKey {
    /// The sealed key.
    pub key: ApiKey,
    /// Where it was found.
    pub source: KeySource,
}

/// Resolves the key of `provider` from its variable ([`env_var`]).
///
/// The process environment is read first. A blank value there counts as set:
/// it skips `.env` and goes to the keyring. An unset variable is looked up in
/// `<repo_root>/.env` (which never overrides a set variable), then in the
/// keyring under service = variable name and account = OS user. Nothing is
/// logged; callers log the returned error.
///
/// # Errors
///
/// [`SecretsError::KeyMissing`] when no source has a non-blank key;
/// [`SecretsError::KeyMalformed`] when the first non-blank key breaks the key
/// rules; [`SecretsError::DotenvRead`] when `.env` exists and cannot be read;
/// [`SecretsError::KeyringRead`] or [`SecretsError::KeyringTimeout`] when the
/// keyring fails. No error contains key bytes.
pub fn resolve(provider: Provider, sources: &KeySources<'_>) -> Result<ResolvedKey, SecretsError> {
    resolve_name(env_var(provider), sources)
}

/// Resolves a key from a variable named by configuration, in [`resolve`]'s order.
///
/// `var` is a local runner's `key_env` (for example `LAYA_API_KEY`); it is the
/// same validated [`VarName`] type as `LocalRunner::key_env`, so a config can
/// only name a well-formed variable.
///
/// # Errors
///
/// As [`resolve`].
pub fn resolve_var(var: &VarName, sources: &KeySources<'_>) -> Result<ResolvedKey, SecretsError> {
    resolve_name(var.as_str(), sources)
}

fn resolve_name(name: &str, sources: &KeySources<'_>) -> Result<ResolvedKey, SecretsError> {
    let process_value = sources.env.var(name);
    if let Some(raw) = process_value.as_deref().filter(|value| is_present(value)) {
        return seal(name, raw.to_owned(), KeySource::Env);
    }

    if process_value.is_none()
        && let Some(repo_root) = sources.repo_root
    {
        let dotenv = load_dotenv(repo_root)?;
        if let Some(raw) = dotenv.get(name).filter(|value| is_present(value)) {
            return seal(name, raw.clone(), KeySource::DotEnv);
        }
    }

    let account = current_account();
    if account.is_empty() {
        return Err(missing(name));
    }
    match sources.keyring.password(name, &account)? {
        Some(raw) if is_present(&raw) => seal(name, raw, KeySource::Keyring),
        _ => Err(missing(name)),
    }
}

fn seal(name: &str, raw: String, source: KeySource) -> Result<ResolvedKey, SecretsError> {
    ApiKey::try_new(name, raw).map(|key| ResolvedKey { key, source })
}

/// Whether a source holds a value worth sealing; a blank value counts as absent.
fn is_present(value: &str) -> bool {
    !value.trim().is_empty()
}

fn missing(name: &str) -> SecretsError {
    SecretsError::KeyMissing {
        var: name.to_owned(),
    }
}
