//! Credential lookup: process env, repo-root `.env`, then keyring.

use std::collections::BTreeMap;
#[cfg(test)]
use std::fmt;

use sc_agent_types::{BackendKeyId, JevError, RepoRoot, ResolvedApiKey};

use crate::dotenv::load_dotenv;
use crate::platform::current_account;

/// Environment variable and default keyring service for TypeSafe.
pub const TYPESAFE_API_KEY_VAR: &str = "TYPESAFE_API_KEY";

/// Environment variable and default keyring service for Anthropic.
pub const ANTHROPIC_API_KEY_VAR: &str = "ANTHROPIC_API_KEY";

/// Environment variable and default keyring service for OpenAI.
pub const OPENAI_API_KEY_VAR: &str = "OPENAI_API_KEY";

/// Optional Anthropic keyring service override.
///
/// When this variable is set to a non-empty value, Anthropic keyring lookups
/// use it as the service name. It does not change the Anthropic environment
/// variable, and other backends ignore it.
pub const ANTHROPIC_KEYCHAIN_SERVICE_VAR: &str = "ANTHROPIC_KEYCHAIN_SERVICE";

/// Reads named variables.
///
/// `None` means unset. `Some` means set, including an empty string. The trait
/// is an open extension point (ADR-SAG-0020) so tests can inject values
/// without changing the process environment. It is intentionally not sealed.
pub trait EnvSource: Send + Sync {
    /// Returns the variable named `name` when it is set.
    fn var(&self, name: &str) -> Option<String>;
}

/// Reads one keyring entry.
///
/// `Ok(None)` means the entry is absent. The trait is an open extension point
/// (ADR-SAG-0020) so tests can inject a store. It is intentionally not sealed.
/// Implementations must not put secret material in errors. A store that cannot
/// be read returns [`JevError::credential_store_invalid`] with a sanitized
/// cause.
pub trait KeyringSource: Send + Sync {
    /// Returns the secret for `service` and `account`.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::ConfigInvalid`] when the store cannot be read.
    /// The error must not include secret bytes or passwords.
    fn password(&self, service: &str, account: &str) -> Result<Option<String>, JevError>;
}

/// Resolves one backend API key.
///
/// Search order is the process environment, then `repo_root/.env`, then the
/// platform keyring. `.env` does not override a variable that is already set.
/// The keyring service is the variable name. The account is the OS user.
/// Anthropic keyring lookups honor [`ANTHROPIC_KEYCHAIN_SERVICE_VAR`].
/// The returned [`ResolvedApiKey`] redacts the secret in `Debug` and `Display`.
///
/// This function does not run git and does not contact the network.
///
/// # Errors
///
/// Returns [`JevError::KeyMissing`] (`KEY_MISSING`) when every source lacks a
/// non-empty key. Returns [`JevError::ConfigInvalid`] when `.env` or the
/// keyring cannot be read, or when the first non-empty key contains interior
/// whitespace or a control character ([`ResolvedApiKey::try_new`]). That
/// error carries a sanitized cause and a recovery hint
/// ([`JevError::credential_store_invalid`]). Neither error includes secret
/// material.
///
/// # Panics
///
/// Panics if the process-global keyring builder lock is poisoned.
///
/// # Examples
///
/// ```no_run
/// use sc_agent_secrets::resolve_key;
/// use sc_agent_types::{BackendKeyId, RepoRoot};
///
/// let root = RepoRoot::new(".");
/// let key = resolve_key(BackendKeyId::TypeSafe, &root)?;
/// assert_eq!(key.to_string(), "[redacted]");
/// # Ok::<(), sc_agent_types::JevError>(())
/// ```
pub fn resolve_key(id: BackendKeyId, repo_root: &RepoRoot) -> Result<ResolvedApiKey, JevError> {
    resolve_key_with(
        id,
        repo_root,
        &crate::ProcessEnv,
        &crate::PlatformKeyring,
        current_account(),
    )
}

/// Resolves one backend API key from injected sources.
///
/// `account` is the keyring user. Precedence matches [`resolve_key`]. Passing
/// an empty account skips the keyring and yields `KEY_MISSING` when no earlier
/// source has a key.
///
/// # Errors
///
/// Returns [`JevError::KeyMissing`] when no source has a non-empty key.
/// Returns [`JevError::ConfigInvalid`] when `.env` or `keyring` cannot be
/// read, or when the selected key fails [`ResolvedApiKey::try_new`].
/// The config error names a sanitized cause and a recovery step. Error text
/// does not include secret material.
///
/// # Examples
///
/// See the crate documentation for an injected miss that returns `KEY_MISSING`.
pub fn resolve_key_with(
    id: BackendKeyId,
    repo_root: &RepoRoot,
    env: &impl EnvSource,
    keyring: &impl KeyringSource,
    account: impl AsRef<str>,
) -> Result<ResolvedApiKey, JevError> {
    let Some(var_name) = env_var_name(id) else {
        return Err(missing(id));
    };
    let process_value = env.var(var_name);
    if let Some(raw) = process_value.clone().filter(|value| is_present(value)) {
        return ResolvedApiKey::try_new(id, raw);
    }

    let key_from_dotenv = process_value.is_none();
    let override_from_dotenv =
        id == BackendKeyId::Anthropic && env.var(ANTHROPIC_KEYCHAIN_SERVICE_VAR).is_none();
    let dotenv = if key_from_dotenv || override_from_dotenv {
        load_dotenv(repo_root.as_path())?
    } else {
        BTreeMap::new()
    };

    if key_from_dotenv && let Some(raw) = dotenv.get(var_name).filter(|value| is_present(value)) {
        return ResolvedApiKey::try_new(id, raw.clone());
    }

    let service = keyring_service(id, env, &dotenv);
    let account = account.as_ref();
    if account.is_empty() {
        return Err(missing(id));
    }
    match keyring.password(&service, account)? {
        Some(raw) if is_present(&raw) => ResolvedApiKey::try_new(id, raw),
        _ => Err(missing(id)),
    }
}

/// Returns the environment variable name for `id`.
#[must_use]
pub(crate) fn env_var_name(id: BackendKeyId) -> Option<&'static str> {
    KEY_ENV_VARS
        .iter()
        .find(|(key, _)| *key == id)
        .map(|(_, name)| *name)
}

/// Environment variable of every credential this crate resolves.
const KEY_ENV_VARS: &[(BackendKeyId, &str)] = &[
    (BackendKeyId::TypeSafe, TYPESAFE_API_KEY_VAR),
    (BackendKeyId::Anthropic, ANTHROPIC_API_KEY_VAR),
    (BackendKeyId::OpenAi, OPENAI_API_KEY_VAR),
];

fn keyring_service(
    id: BackendKeyId,
    env: &impl EnvSource,
    dotenv: &BTreeMap<String, String>,
) -> String {
    if id == BackendKeyId::Anthropic {
        match env.var(ANTHROPIC_KEYCHAIN_SERVICE_VAR) {
            Some(value) => {
                if let Some(service) = usable_secret(&value) {
                    return service;
                }
            }
            None => {
                if let Some(service) = dotenv
                    .get(ANTHROPIC_KEYCHAIN_SERVICE_VAR)
                    .and_then(|value| usable_secret(value))
                {
                    return service;
                }
            }
        }
    }
    env_var_name(id).unwrap_or_default().to_owned()
}

/// Whether a source holds a value worth sealing.
///
/// A blank value counts as absent so the next source is tried. Everything
/// else goes to [`ResolvedApiKey::try_new`], which owns the normalization
/// rules.
fn is_present(value: &str) -> bool {
    !value.trim().is_empty()
}

fn usable_secret(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn missing(id: BackendKeyId) -> JevError {
    JevError::key_missing(id)
}

/// In-memory environment map for tests.
///
/// [`Debug`] prints variable names and omits values.
#[cfg(test)]
#[derive(Clone, Default)]
pub struct MapEnv {
    vars: BTreeMap<String, String>,
}

#[cfg(test)]
impl MapEnv {
    /// Builds a map from `(name, value)` pairs.
    #[must_use]
    pub fn with<K, V, I>(pairs: I) -> Self
    where
        K: Into<String>,
        V: Into<String>,
        I: IntoIterator<Item = (K, V)>,
    {
        Self {
            vars: pairs
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        }
    }
}

#[cfg(test)]
impl fmt::Debug for MapEnv {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MapEnv")
            .field("names", &self.vars.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
impl EnvSource for MapEnv {
    fn var(&self, name: &str) -> Option<String> {
        self.vars.get(name).cloned()
    }
}

/// In-memory keyring for tests.
///
/// Entries are visible only for the account given at construction. [`Debug`]
/// prints the account and service names and omits secrets.
#[cfg(test)]
#[derive(Clone)]
pub struct MapKeyring {
    account: String,
    entries: BTreeMap<String, String>,
}

#[cfg(test)]
impl MapKeyring {
    /// Builds a store whose secrets are returned only for `account`.
    #[must_use]
    pub fn new<S, V, I>(account: impl Into<String>, entries: I) -> Self
    where
        S: Into<String>,
        V: Into<String>,
        I: IntoIterator<Item = (S, V)>,
    {
        Self {
            account: account.into(),
            entries: entries
                .into_iter()
                .map(|(service, secret)| (service.into(), secret.into()))
                .collect(),
        }
    }
}

#[cfg(test)]
impl fmt::Debug for MapKeyring {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MapKeyring")
            .field("account", &self.account)
            .field("services", &self.entries.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
impl KeyringSource for MapKeyring {
    fn password(&self, service: &str, account: &str) -> Result<Option<String>, JevError> {
        if account != self.account {
            return Ok(None);
        }
        Ok(self.entries.get(service).cloned())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::dotenv::{DOTENV_RECOVERY, DOTENV_UNREADABLE};
    use crate::platform::{KEYRING_RECOVERY, KEYRING_UNREADABLE};
    use crate::test_support::TempRepo;
    use sc_agent_types::ResolvedApiKey;

    const SECRET: &str = "super-secret-value";

    /// Expected key, compared by value so tests never read the secret.
    fn sealed(secret: &str) -> ResolvedApiKey {
        ResolvedApiKey::try_new(BackendKeyId::TypeSafe, secret.to_owned()).expect("test key")
    }

    fn assert_no_secret(error: &JevError, secret: &str) {
        let rendered = format!("{error} {error:?} {}", error.fix_hint());
        assert!(!rendered.contains(secret), "{rendered}");
        assert!(!rendered.to_ascii_lowercase().contains("bearer "));
    }

    #[test]
    fn test_api_key_env_first_then_keychain() {
        let repo = TempRepo::new(Some("OPENAI_API_KEY=from-dotenv\n"));
        let env = MapEnv::with([("OPENAI_API_KEY", "from-env")]);
        let keyring = MapKeyring::new("tester", [("OPENAI_API_KEY", "from-keyring")]);
        let key =
            resolve_key_with(BackendKeyId::OpenAi, &repo.root(), &env, &keyring, "tester").unwrap();
        assert_eq!(key, sealed("from-env"));
    }

    #[test]
    fn test_jev_key_env_first_then_keychain() {
        let repo = TempRepo::new(Some("TYPESAFE_API_KEY=from-dotenv\n"));
        let env = MapEnv::with([("TYPESAFE_API_KEY", "from-env")]);
        let keyring = MapKeyring::new("tester", [("TYPESAFE_API_KEY", "from-keyring")]);
        let key = resolve_key_with(
            BackendKeyId::TypeSafe,
            &repo.root(),
            &env,
            &keyring,
            "tester",
        )
        .unwrap();
        assert_eq!(key, sealed("from-env"));
    }

    #[test]
    fn test_api_key_missing_sends_nothing() {
        let repo = TempRepo::new(Some(&format!("OTHER={SECRET}\n")));
        let env = MapEnv::with([("UNRELATED_SECRET", SECRET)]);
        let keyring = MapKeyring::new("tester", [("OTHER_SERVICE", SECRET)]);
        let error = resolve_key_with(
            BackendKeyId::Anthropic,
            &repo.root(),
            &env,
            &keyring,
            "tester",
        )
        .unwrap_err();
        assert_eq!(error.code(), "KEY_MISSING");
        assert!(error.fix_hint().contains("keyring"));
        assert_no_secret(&error, SECRET);
        assert!(matches!(
            error,
            JevError::KeyMissing {
                backend: BackendKeyId::Anthropic,
                ..
            }
        ));
    }

    #[test]
    fn blank_process_env_blocks_dotenv_and_uses_keyring() {
        let repo = TempRepo::new(Some("TYPESAFE_API_KEY=from-dotenv\n"));
        let env = MapEnv::with([("TYPESAFE_API_KEY", "  ")]);
        let keyring = MapKeyring::new("tester", [("TYPESAFE_API_KEY", "from-keyring")]);
        let key = resolve_key_with(
            BackendKeyId::TypeSafe,
            &repo.root(),
            &env,
            &keyring,
            "tester",
        )
        .unwrap();
        assert_eq!(key, sealed("from-keyring"));
    }

    #[test]
    fn anthropic_keychain_service_override_is_anthropic_only() {
        let repo = TempRepo::new(None);
        let env = MapEnv::with([("ANTHROPIC_KEYCHAIN_SERVICE", "custom-service")]);
        let keyring = MapKeyring::new(
            "tester",
            [
                ("custom-service", "from-custom"),
                ("ANTHROPIC_API_KEY", "from-default"),
                ("OPENAI_API_KEY", "from-openai"),
            ],
        );
        let anthropic = resolve_key_with(
            BackendKeyId::Anthropic,
            &repo.root(),
            &env,
            &keyring,
            "tester",
        )
        .unwrap();
        assert_eq!(anthropic, sealed("from-custom"));
        let openai =
            resolve_key_with(BackendKeyId::OpenAi, &repo.root(), &env, &keyring, "tester").unwrap();
        assert_eq!(openai, sealed("from-openai"));
    }

    #[test]
    fn anthropic_override_comes_from_dotenv_when_unset() {
        let repo = TempRepo::new(Some("ANTHROPIC_KEYCHAIN_SERVICE=from-dotenv-service\n"));
        let keyring = MapKeyring::new(
            "alice",
            [
                ("from-dotenv-service", "from-custom"),
                ("ANTHROPIC_API_KEY", "from-default"),
            ],
        );
        let key = resolve_key_with(
            BackendKeyId::Anthropic,
            &repo.root(),
            &MapEnv::default(),
            &keyring,
            "alice",
        )
        .unwrap();
        assert_eq!(key, sealed("from-custom"));
    }

    #[test]
    fn process_override_wins_over_dotenv_override() {
        let repo = TempRepo::new(Some("ANTHROPIC_KEYCHAIN_SERVICE=from-dotenv-service\n"));
        let env = MapEnv::with([("ANTHROPIC_KEYCHAIN_SERVICE", "from-env-service")]);
        let keyring = MapKeyring::new(
            "tester",
            [
                ("from-env-service", "from-env-ring"),
                ("from-dotenv-service", "from-dotenv-ring"),
            ],
        );
        let key = resolve_key_with(
            BackendKeyId::Anthropic,
            &repo.root(),
            &env,
            &keyring,
            "tester",
        )
        .unwrap();
        assert_eq!(key, sealed("from-env-ring"));
    }

    #[test]
    fn wrong_keyring_account_misses() {
        let repo = TempRepo::new(None);
        let keyring = MapKeyring::new("alice", [("OPENAI_API_KEY", "from-keyring")]);
        let error = resolve_key_with(
            BackendKeyId::OpenAi,
            &repo.root(),
            &MapEnv::default(),
            &keyring,
            "bob",
        )
        .unwrap_err();
        assert_eq!(error.code(), "KEY_MISSING");
        assert_no_secret(&error, "from-keyring");
    }

    #[test]
    fn unreadable_dotenv_is_config_invalid_without_secrets() {
        let repo = TempRepo::new(None);
        fs::create_dir(repo.path().join(".env")).unwrap();
        let env = MapEnv::with([("UNRELATED_SECRET", SECRET)]);
        let keyring = MapKeyring::new("tester", [("OPENAI_API_KEY", SECRET)]);
        let error = resolve_key_with(BackendKeyId::OpenAi, &repo.root(), &env, &keyring, "tester")
            .unwrap_err();
        assert_eq!(error.code(), "CONFIG_INVALID");
        let hint = error.fix_hint();
        assert!(hint.contains(DOTENV_UNREADABLE), "{hint}");
        assert!(hint.contains("cause:"), "{hint}");
        assert!(hint.contains(DOTENV_RECOVERY), "{hint}");
        assert!(!hint.contains("check.toml"), "{hint}");
        assert_no_secret(&error, SECRET);
    }

    #[test]
    fn process_env_key_ignores_unreadable_dotenv() {
        let repo = TempRepo::new(None);
        fs::create_dir(repo.path().join(".env")).unwrap();
        let env = MapEnv::with([("OPENAI_API_KEY", "from-env")]);
        let keyring = MapKeyring::new("tester", Vec::<(&str, &str)>::new());
        let key =
            resolve_key_with(BackendKeyId::OpenAi, &repo.root(), &env, &keyring, "tester").unwrap();
        assert_eq!(key, sealed("from-env"));
    }

    #[test]
    fn keyring_failure_does_not_echo_secrets() {
        struct Unavailable;
        impl KeyringSource for Unavailable {
            fn password(&self, service: &str, _account: &str) -> Result<Option<String>, JevError> {
                let _ = service;
                Err(JevError::credential_store_invalid(
                    KEYRING_UNREADABLE,
                    "platform keystore returned an error",
                    KEYRING_RECOVERY,
                ))
            }
        }
        let repo = TempRepo::new(None);
        let error = resolve_key_with(
            BackendKeyId::TypeSafe,
            &repo.root(),
            &MapEnv::default(),
            &Unavailable,
            "tester",
        )
        .unwrap_err();
        assert_eq!(error.code(), "CONFIG_INVALID");
        let hint = error.fix_hint();
        assert!(hint.contains(KEYRING_UNREADABLE), "{hint}");
        assert!(
            hint.contains("cause: platform keystore returned an error"),
            "{hint}"
        );
        assert!(hint.contains(KEYRING_RECOVERY), "{hint}");
        assert!(!hint.contains("check.toml"), "{hint}");
        assert!(!hint.contains(SECRET), "{hint}");
        assert_no_secret(&error, SECRET);
    }

    #[test]
    fn debug_omits_secret_values() {
        let env = MapEnv::with([("TYPESAFE_API_KEY", SECRET)]);
        let rendered = format!("{env:?}");
        assert!(rendered.contains("TYPESAFE_API_KEY"));
        assert!(!rendered.contains(SECRET));

        let keyring = MapKeyring::new("tester", [("OPENAI_API_KEY", SECRET)]);
        let rendered = format!("{keyring:?}");
        assert!(rendered.contains("OPENAI_API_KEY"));
        assert!(rendered.contains("tester"));
        assert!(!rendered.contains(SECRET));

        let resolved = sealed(SECRET);
        let rendered = format!("{resolved:?} {resolved}");
        assert!(rendered.contains("[redacted]"));
        assert!(!rendered.contains(SECRET));
        assert_eq!(resolved, sealed(SECRET));
    }

    #[test]
    fn keys_are_normalized_once_by_try_new() {
        let repo = TempRepo::new(Some("OPENAI_API_KEY=  from-dotenv  \n"));
        let keyring = MapKeyring::new("tester", [("TYPESAFE_API_KEY", " from-keyring\n")]);
        let trimmed = resolve_key_with(
            BackendKeyId::OpenAi,
            &repo.root(),
            &MapEnv::default(),
            &keyring,
            "tester",
        )
        .unwrap();
        assert_eq!(trimmed, sealed("from-dotenv"));
        let from_ring = resolve_key_with(
            BackendKeyId::TypeSafe,
            &repo.root(),
            &MapEnv::default(),
            &keyring,
            "tester",
        )
        .unwrap();
        assert_eq!(from_ring, sealed("from-keyring"));

        for (source_env, source_dotenv) in [
            (Some(format!("{SECRET} pasted twice")), None),
            (None, Some(format!("ANTHROPIC_API_KEY={SECRET}\tx\n"))),
        ] {
            let repo = TempRepo::new(source_dotenv.as_deref());
            let env = MapEnv::with(
                source_env
                    .into_iter()
                    .map(|value| ("ANTHROPIC_API_KEY".to_owned(), value)),
            );
            let error = resolve_key_with(
                BackendKeyId::Anthropic,
                &repo.root(),
                &env,
                &MapKeyring::new("tester", Vec::<(&str, &str)>::new()),
                "tester",
            )
            .unwrap_err();
            assert_eq!(error.code(), "CONFIG_INVALID");
            assert!(
                error
                    .fix_hint()
                    .contains("whitespace or a control character")
            );
            assert_no_secret(&error, SECRET);
        }
    }

    #[test]
    fn sources_are_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MapEnv>();
        assert_send_sync::<MapKeyring>();
        assert_send_sync::<crate::ProcessEnv>();
        assert_send_sync::<crate::PlatformKeyring>();
    }

    #[test]
    fn resolution_does_not_mutate_process_env() {
        let name = "SC_AGENT_SECRETS_RESOLVE_PROBE";
        let before = std::env::var(name);
        let repo = TempRepo::new(Some(&format!(
            "{name}=from-dotenv\nOPENAI_API_KEY=from-dotenv\n"
        )));
        let key = resolve_key_with(
            BackendKeyId::OpenAi,
            &repo.root(),
            &MapEnv::default(),
            &MapKeyring::new("tester", Vec::<(&str, &str)>::new()),
            "tester",
        )
        .unwrap();
        assert_eq!(key, sealed("from-dotenv"));
        assert_eq!(std::env::var(name).ok(), before.ok());
    }
}
