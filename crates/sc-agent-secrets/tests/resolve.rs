//! Key resolution order and redaction (REQ-SAG-0401, REQ-SAG-0002).
//!
//! Every test injects its environment ([`MapEnv`]) and keyring
//! ([`MapKeyring`]) and writes `.env` files only under a temp dir: the process
//! environment, the real keyring and HOME are never touched.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

use sc_agent_secrets::{
    ALL_VARS, ApiKey, EnvSource, KeySource, KeySources, KeyringSource, LAYA_API_KEY, ProcessEnv,
    ResolvedKey, SecretsError, env_var, resolve, resolve_var,
};
use sc_agent_types::{ErrorId, Provider, VarName};

/// In-memory environment map for tests.
///
/// [`Debug`] prints variable names and omits values.
#[derive(Clone, Default)]
pub struct MapEnv {
    vars: BTreeMap<String, String>,
}

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

impl fmt::Debug for MapEnv {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MapEnv")
            .field("names", &self.vars.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl EnvSource for MapEnv {
    fn var(&self, name: &str) -> Option<String> {
        self.vars.get(name).cloned()
    }
}
/// In-memory keyring for tests.
///
/// Every lookup is recorded as `(service, account)` so a test can check the
/// service is the variable name and the account the OS user. [`Debug`]
/// prints the service names and omits secrets.
#[derive(Default)]
pub struct MapKeyring {
    entries: BTreeMap<String, String>,
    asked: Mutex<Vec<(String, String)>>,
}

impl MapKeyring {
    /// Builds a store from `(service, secret)` pairs.
    #[must_use]
    pub fn new<S, V, I>(entries: I) -> Self
    where
        S: Into<String>,
        V: Into<String>,
        I: IntoIterator<Item = (S, V)>,
    {
        Self {
            entries: entries
                .into_iter()
                .map(|(service, secret)| (service.into(), secret.into()))
                .collect(),
            asked: Mutex::new(Vec::new()),
        }
    }

    /// The `(service, account)` pairs looked up so far.
    pub fn asked(&self) -> Vec<(String, String)> {
        self.asked
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl fmt::Debug for MapKeyring {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MapKeyring")
            .field("services", &self.entries.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl KeyringSource for MapKeyring {
    fn password(&self, service: &str, account: &str) -> Result<Option<String>, SecretsError> {
        self.asked
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((service.to_owned(), account.to_owned()));
        Ok(self.entries.get(service).cloned())
    }
}

/// A keyring that always fails, with secret bytes in the inner text it never reports.
struct FailingKeyring;

impl KeyringSource for FailingKeyring {
    fn password(&self, _service: &str, _account: &str) -> Result<Option<String>, SecretsError> {
        Err(SecretsError::KeyringRead {
            kind: "platform_failure".to_owned(),
            cause: "platform keystore returned an error",
        })
    }
}

const SECRET: &str = "sk-test-secret-123";
const OTHER: &str = "sk-other-secret-456";

/// Whether `key` holds exactly `expected`, without reading the key.
fn holds(key: &ApiKey, expected: &str) -> bool {
    key.redact_text(expected) == "[redacted]"
}

fn sources<'a>(
    env: &'a MapEnv,
    repo_root: Option<&'a Path>,
    keyring: &'a dyn KeyringSource,
) -> KeySources<'a> {
    KeySources {
        env,
        repo_root,
        keyring,
    }
}

fn repo_with_dotenv(text: &str) -> std::io::Result<tempfile::TempDir> {
    let repo = tempfile::tempdir()?;
    fs::write(repo.path().join(".env"), text)?;
    Ok(repo)
}

fn assert_found(resolved: &ResolvedKey, expected: &str, source: KeySource) {
    assert_eq!(resolved.source, source);
    assert!(holds(&resolved.key, expected), "wrong key from {source:?}");
}

/// AC REQ-SAG-0401: env first, then `.env`, then the keyring, for each of the six providers.
#[test]
fn every_provider_resolves_env_then_dotenv_then_keyring() {
    for provider in Provider::ALL.iter().copied() {
        let var = env_var(provider);
        let repo = repo_with_dotenv(&format!("{var}={OTHER}\n")).unwrap();
        let keyring = MapKeyring::new([(var, "sk-keyring-only")]);

        // Env wins over .env and keyring; .env does not override a set variable.
        let env = MapEnv::with([(var, SECRET)]);
        let found = resolve(provider, &sources(&env, Some(repo.path()), &keyring)).unwrap();
        assert_found(&found, SECRET, KeySource::Env);
        assert!(keyring.asked().is_empty(), "keyring read after an env hit");

        // Unset in env: .env supplies it.
        let empty = MapEnv::default();
        let found = resolve(provider, &sources(&empty, Some(repo.path()), &keyring)).unwrap();
        assert_found(&found, OTHER, KeySource::DotEnv);
        assert!(keyring.asked().is_empty(), "keyring read after a .env hit");

        // Neither env nor .env: the keyring, service = variable name.
        let bare = tempfile::tempdir().unwrap();
        let found = resolve(provider, &sources(&empty, Some(bare.path()), &keyring)).unwrap();
        assert_found(&found, "sk-keyring-only", KeySource::Keyring);
        let asked = keyring.asked();
        assert_eq!(asked.len(), 1);
        let (service, account) = asked.first().unwrap();
        assert_eq!(service, var);
        assert!(!account.is_empty(), "account is the OS user");
    }
}

/// AC REQ-SAG-0401: none -> secrets.key_missing naming the variable, with recovery text.
#[test]
fn no_key_anywhere_is_key_missing_with_recovery() {
    for provider in Provider::ALL.iter().copied() {
        let var = env_var(provider);
        let repo = repo_with_dotenv("UNRELATED=value\n").unwrap();
        let keyring = MapKeyring::default();
        let env = MapEnv::default();
        let error = resolve(provider, &sources(&env, Some(repo.path()), &keyring)).unwrap_err();
        assert_eq!(
            error,
            SecretsError::KeyMissing {
                var: var.to_owned()
            }
        );
        assert_eq!(error.id(), "secrets.key_missing");
        assert!(error.message().contains(var));
        let recovery = error.recovery().unwrap();
        assert!(!recovery.is_empty());
        for place in ["process environment", ".env", "keyring"] {
            assert!(recovery.contains(place), "{recovery}");
        }
        assert_eq!(error.detail().recovery.as_deref(), Some(recovery));
    }
}

/// AC REQ-SAG-0401: a malformed key -> secrets.key_malformed, from any source.
#[test]
fn malformed_key_is_key_malformed_without_key_bytes() {
    let var = env_var(Provider::Anthropic);
    let bad = format!("{SECRET} tail");
    let env = MapEnv::with([(var, bad.as_str())]);
    let keyring = MapKeyring::default();
    let error = resolve(Provider::Anthropic, &sources(&env, None, &keyring)).unwrap_err();
    assert_eq!(error.id(), "secrets.key_malformed");
    assert_no_secret(&error, SECRET);

    let empty = MapEnv::default();
    let keyring = MapKeyring::new([(var, format!("{SECRET}\u{7}"))]);
    let error = resolve(Provider::Anthropic, &sources(&empty, None, &keyring)).unwrap_err();
    assert_eq!(error.id(), "secrets.key_malformed");
    assert_no_secret(&error, SECRET);
}

/// A blank env value counts as set: it skips `.env` and goes to the keyring.
#[test]
fn blank_env_value_skips_dotenv_and_reads_keyring() {
    let var = env_var(Provider::OpenAi);
    let repo = repo_with_dotenv(&format!("{var}={OTHER}\n")).unwrap();
    let env = MapEnv::with([(var, "  ")]);
    let keyring = MapKeyring::new([(var, SECRET)]);
    let found = resolve(
        Provider::OpenAi,
        &sources(&env, Some(repo.path()), &keyring),
    )
    .unwrap();
    assert_found(&found, SECRET, KeySource::Keyring);

    let empty_keyring = MapKeyring::default();
    let error = resolve(
        Provider::OpenAi,
        &sources(&env, Some(repo.path()), &empty_keyring),
    )
    .unwrap_err();
    assert_eq!(error.id(), "secrets.key_missing");
}

/// `repo_root: None` reads no `.env`.
#[test]
fn no_repo_root_reads_no_dotenv() {
    let var = env_var(Provider::TypeSafe);
    let _repo = repo_with_dotenv(&format!("{var}={OTHER}\n")).unwrap();
    let env = MapEnv::default();
    let keyring = MapKeyring::default();
    let error = resolve(Provider::TypeSafe, &sources(&env, None, &keyring)).unwrap_err();
    assert_eq!(error.id(), "secrets.key_missing");
}

/// An unreadable `.env` (here a directory) -> secrets.dotenv_read.
#[test]
fn unreadable_dotenv_is_dotenv_read() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir(repo.path().join(".env")).unwrap();
    let env = MapEnv::default();
    let keyring = MapKeyring::default();
    let error = resolve(Provider::Nous, &sources(&env, Some(repo.path()), &keyring)).unwrap_err();
    assert_eq!(error.id(), "secrets.dotenv_read");
    assert!(error.recovery().is_none());
    assert!(
        keyring.asked().is_empty(),
        "a .env failure stops resolution"
    );
}

/// A failing keyring -> secrets.keyring_read, with no secret bytes in the error.
#[test]
fn failing_keyring_is_keyring_read() {
    let env = MapEnv::default();
    let error = resolve(Provider::Together, &sources(&env, None, &FailingKeyring)).unwrap_err();
    assert_eq!(error.id(), "secrets.keyring_read");
    assert_no_secret(&error, SECRET);
}

/// AC REQ-SAG-0401: resolve_var uses the same order for a configured variable.
#[test]
fn resolve_var_reads_a_configured_variable() {
    let laya: VarName = LAYA_API_KEY.parse().unwrap();
    let env = MapEnv::with([(LAYA_API_KEY, SECRET)]);
    let keyring = MapKeyring::default();
    let found = resolve_var(&laya, &sources(&env, None, &keyring)).unwrap();
    assert_found(&found, SECRET, KeySource::Env);

    let custom: VarName = "MY_RUNNER_TOKEN".parse().unwrap();
    let repo = repo_with_dotenv(&format!("MY_RUNNER_TOKEN={OTHER}\n")).unwrap();
    let empty = MapEnv::default();
    let found = resolve_var(&custom, &sources(&empty, Some(repo.path()), &keyring)).unwrap();
    assert_found(&found, OTHER, KeySource::DotEnv);

    let keyring = MapKeyring::new([("MY_RUNNER_TOKEN", SECRET)]);
    let found = resolve_var(&custom, &sources(&empty, None, &keyring)).unwrap();
    assert_found(&found, SECRET, KeySource::Keyring);

    let error = resolve_var(&custom, &sources(&empty, None, &MapKeyring::default())).unwrap_err();
    assert_eq!(
        error,
        SecretsError::KeyMissing {
            var: "MY_RUNNER_TOKEN".to_owned()
        }
    );
}

#[test]
fn all_vars_holds_every_provider_variable_and_laya() {
    for provider in Provider::ALL.iter().copied() {
        assert!(ALL_VARS.contains(&env_var(provider)), "{provider}");
    }
    assert!(ALL_VARS.contains(&LAYA_API_KEY));
    assert_eq!(ALL_VARS.len(), 7);
    let distinct: std::collections::BTreeSet<_> = ALL_VARS.iter().collect();
    assert_eq!(distinct.len(), ALL_VARS.len());
}

#[test]
fn key_source_spellings() {
    assert_eq!(KeySource::Env.as_str(), "env");
    assert_eq!(KeySource::DotEnv.as_str(), ".env");
    assert_eq!(KeySource::Keyring.as_str(), "keyring");
}

#[test]
fn sources_and_keys_are_send_and_sync() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<ApiKey>();
    send_sync::<MapEnv>();
    send_sync::<ProcessEnv>();
    send_sync::<SecretsError>();
}

/// AC "never log or print a key": ApiKey prints [redacted]; no error text holds key bytes.
#[test]
fn never_print_a_key() {
    let var = env_var(Provider::OpenRouter);
    let env = MapEnv::with([(var, SECRET)]);
    let keyring = MapKeyring::default();
    let found = resolve(Provider::OpenRouter, &sources(&env, None, &keyring)).unwrap();
    assert_eq!(format!("{:?}", found.key), "ApiKey([redacted])");
    assert_eq!(found.key.to_string(), "[redacted]");
    let rendered = format!("{found:?}");
    assert!(!rendered.contains(SECRET), "{rendered}");

    let errors = [
        SecretsError::KeyMissing {
            var: var.to_owned(),
        },
        resolve(
            Provider::OpenRouter,
            &sources(
                &MapEnv::with([(var, format!("{SECRET}\n\tx"))]),
                None,
                &keyring,
            ),
        )
        .unwrap_err(),
        resolve(
            Provider::OpenRouter,
            &sources(&MapEnv::with([(var, format!("{SECRET}é"))]), None, &keyring),
        )
        .unwrap_err(),
        resolve(
            Provider::OpenRouter,
            &sources(&MapEnv::default(), None, &FailingKeyring),
        )
        .unwrap_err(),
        SecretsError::KeyringTimeout,
    ];
    for error in &errors {
        assert_no_secret(error, SECRET);
    }
}

/// AC REQ-SAG-0002 (secrets half): ids secrets.*, Display "<id>: <message>", no cause.
#[test]
fn secrets_errors_follow_the_error_contract() {
    let errors = [
        SecretsError::KeyMissing {
            var: "MY_RUNNER_TOKEN".to_owned(),
        },
        SecretsError::KeyMalformed {
            var: "MY_RUNNER_TOKEN".to_owned(),
            reason: "key contains whitespace or a control character",
        },
        SecretsError::DotenvRead {
            kind: "PermissionDenied".to_owned(),
            cause: "permission denied",
        },
        SecretsError::KeyringRead {
            kind: "platform_failure".to_owned(),
            cause: "platform keystore returned an error",
        },
        SecretsError::KeyringTimeout,
    ];
    let ids: Vec<&str> = errors.iter().map(ErrorId::id).collect();
    assert_eq!(
        ids,
        [
            "secrets.key_missing",
            "secrets.key_malformed",
            "secrets.dotenv_read",
            "secrets.keyring_read",
            "secrets.keyring_timeout",
        ]
    );
    for error in &errors {
        assert_eq!(
            error.to_string(),
            format!("{}: {}", error.id(), error.message())
        );
        assert!(error.cause_detail().is_none());
        assert!(std::error::Error::source(error).is_none());
        let detail = error.detail();
        assert_eq!(detail.id, error.id());
        assert_eq!(detail.message, error.message());
    }
    assert!(
        errors
            .first()
            .unwrap()
            .recovery()
            .is_some_and(|text| !text.is_empty())
    );
    assert!(
        errors
            .get(1)
            .unwrap()
            .recovery()
            .is_some_and(|text| !text.is_empty())
    );
    for error in errors.iter().skip(2) {
        assert!(error.recovery().is_none());
    }
}

fn assert_no_secret(error: &SecretsError, secret: &str) {
    let rendered = format!("{error} {error:?} {:?}", error.detail());
    assert!(!rendered.contains(secret), "{rendered}");
}
