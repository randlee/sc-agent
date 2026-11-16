//! Key resolution: repo-root `.env`, then process environment, then keyring.

use std::path::Path;

use sc_agent_types::{Provider, VarName};

use crate::api_key::ApiKey;
use crate::dotenv::load_dotenv;
use crate::env::{EnvSource, ProcessEnv};
use crate::error::SecretsError;
use crate::keyring::{DeferredPlatformKeyring, KeyringSource, current_account};
use crate::vars::env_var;

/// Where keys are looked up, in this order (REQ-SAG-0401).
///
/// The repo-root `.env` first, then the process environment, then the keyring.
/// Tests build one from their own [`EnvSource`] and [`KeyringSource`], so no
/// test reads or writes the process environment or the real keyring.
pub struct KeySources<'a> {
    /// The environment read second.
    pub env: &'a dyn EnvSource,
    /// The git work-tree top level whose `.env` is read first; `None` reads no `.env`.
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
            keyring: &SYSTEM_KEYRING,
        }
    }
}

static SYSTEM_KEYRING: DeferredPlatformKeyring = DeferredPlatformKeyring;

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

/// A key and the source that supplied it (`decidr doctor` lists the source).
#[derive(Debug, Clone)]
pub struct ResolvedKey {
    /// The sealed key.
    pub key: ApiKey,
    /// Where it was found.
    pub source: KeySource,
}

/// Resolves the key of `provider` from its variable ([`env_var`]).
///
/// The repository-root `.env` is read first, then the process environment,
/// then the keyring under service = variable name and account = OS user.
/// A present but invalid value is authoritative and never falls back.
/// Nothing is logged; callers log the returned error.
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
    // Both entry points meet at a `&str`: provider names are `&'static str`
    // constants (A-1's VarName has no const constructor, so they cannot be
    // VarName values without a parse whose error cannot happen); the test
    // `every_credential_name_is_a_var_name` proves each constant would parse.
    resolve_name(var.as_str(), sources)
}

fn resolve_name(name: &str, sources: &KeySources<'_>) -> Result<ResolvedKey, SecretsError> {
    // A selected value is authoritative, including a blank or malformed one.
    // Never consult a lower-priority store after a value has been found.
    if let Some(repo_root) = sources.repo_root {
        let dotenv = load_dotenv(repo_root)?;
        if let Some(raw) = dotenv.get(name) {
            return seal(name, raw, KeySource::DotEnv);
        }
    }
    if let Some(raw) = sources.env.var(name) {
        return seal(name, &raw, KeySource::Env);
    }

    let account = current_account();
    if account.is_empty() {
        return Err(missing(name));
    }
    match sources.keyring.password(name, &account)? {
        Some(raw) if is_present(&raw) => seal(name, &raw, KeySource::Keyring),
        _ => Err(missing(name)),
    }
}

fn seal(name: &str, raw: &str, source: KeySource) -> Result<ResolvedKey, SecretsError> {
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
