//! Resolves backend API keys for sc-agent.
//!
//! Precedence is the process environment, then the repo-root `.env` file,
//! then the platform keyring. The keyring service is the environment variable
//! name and the account is the OS user. Anthropic keyring lookups use
//! `ANTHROPIC_KEYCHAIN_SERVICE` when that variable is set.
//!
//! Callers pass the git toplevel as a [`RepoRoot`]. This crate does not run
//! git, does not open network connections, and does not log credential values.
//! Errors are [`JevError`] values and never include secret material
//! (ADR-SAG-0003). Log lines stay under NFR-SAG-0002 and ADR-SAG-0013; this
//! crate emits none.
//!
//! [`resolve_key`] returns a [`ResolvedApiKey`]. Its `Debug` and `Display`
//! implementations redact the secret (ADR-SAG-0020).
//!
//! [`EnvSource`] and [`KeyringSource`] are open injection traits. The
//! `test-util` feature exposes `MapEnv` and `MapKeyring` for downstream tests.
//! Production builds leave that feature off.
//!
//! # Examples
//!
//! ```
//! use sc_agent_secrets::{resolve_key_with, EnvSource, KeyringSource};
//! use sc_agent_types::{BackendKeyId, JevError, RepoRoot};
//!
//! struct EmptyEnv;
//! impl EnvSource for EmptyEnv {
//!     fn var(&self, _name: &str) -> Option<String> {
//!         None
//!     }
//! }
//!
//! struct EmptyKeyring;
//! impl KeyringSource for EmptyKeyring {
//!     fn password(&self, _service: &str, _account: &str) -> Result<Option<String>, JevError> {
//!         Ok(None)
//!     }
//! }
//!
//! let root = RepoRoot::new("/this/path/does/not/exist/sc-agent-secrets-doctest");
//! let error = resolve_key_with(
//!     BackendKeyId::OpenAi,
//!     &root,
//!     &EmptyEnv,
//!     &EmptyKeyring,
//!     "nobody",
//! )
//! .expect_err("missing key");
//! assert_eq!(error.code(), "KEY_MISSING");
//! ```

#![warn(missing_docs)]

mod dotenv;
mod platform;
mod resolve;

#[cfg(test)]
mod test_support;

#[doc(inline)]
pub use platform::{PlatformKeyring, ProcessEnv};
#[doc(inline)]
pub use resolve::{
    ANTHROPIC_API_KEY_VAR, ANTHROPIC_KEYCHAIN_SERVICE_VAR, EnvSource, KeyringSource,
    OPENAI_API_KEY_VAR, ResolvedApiKey, TYPESAFE_API_KEY_VAR, resolve_key, resolve_key_with,
};

#[cfg(any(test, feature = "test-util"))]
#[doc(inline)]
pub use resolve::{MapEnv, MapKeyring};

// Crate-root name so `cargo test -- test_env_dotenv_keychain_order --exact` selects it.
#[cfg(test)]
#[test]
fn test_env_dotenv_keychain_order() {
    use sc_agent_types::BackendKeyId;

    use crate::test_support::TempRepo;
    use crate::{MapEnv, MapKeyring, resolve_key_with};

    let repo = TempRepo::new(Some(
        "TYPESAFE_API_KEY=from-dotenv\nANTHROPIC_API_KEY=from-dotenv-anthropic\n",
    ));
    let env = MapEnv::with([("TYPESAFE_API_KEY", "from-env")]);
    let keyring = MapKeyring::new(
        "tester",
        [
            ("TYPESAFE_API_KEY", "from-keyring"),
            ("ANTHROPIC_API_KEY", "from-keyring-anthropic"),
            ("OPENAI_API_KEY", "from-keyring-openai"),
        ],
    );

    let from_env = resolve_key_with(
        BackendKeyId::TypeSafe,
        &repo.root(),
        &env,
        &keyring,
        "tester",
    )
    .unwrap();
    assert_eq!(from_env.as_str(), "from-env");

    let from_dotenv = resolve_key_with(
        BackendKeyId::Anthropic,
        &repo.root(),
        &env,
        &keyring,
        "tester",
    )
    .unwrap();
    assert_eq!(from_dotenv.as_str(), "from-dotenv-anthropic");

    let from_keyring =
        resolve_key_with(BackendKeyId::OpenAi, &repo.root(), &env, &keyring, "tester").unwrap();
    assert_eq!(from_keyring.as_str(), "from-keyring-openai");

    let empty = TempRepo::new(None);
    let missing = resolve_key_with(
        BackendKeyId::TypeSafe,
        &empty.root(),
        &MapEnv::default(),
        &MapKeyring::new("tester", Vec::<(&str, &str)>::new()),
        "tester",
    )
    .unwrap_err();
    assert_eq!(missing.code(), "KEY_MISSING");
    assert!(!missing.fix_hint().is_empty());
    for secret in ["from-env", "from-dotenv", "from-keyring"] {
        let rendered = format!("{missing} {missing:?} {}", missing.fix_hint());
        assert!(!rendered.contains(secret), "{rendered}");
    }
}
