//! Resolves backend API keys for sc-agent.
//!
//! Precedence is the process environment, then the repo-root `.env` file,
//! then the platform keyring. The keyring service is the environment variable
//! name and the account is the OS user. Anthropic keyring lookups use
//! `ANTHROPIC_KEYCHAIN_SERVICE` when that variable is set.
//!
//! Callers pass the git toplevel as a
//! [`RepoRoot`](sc_agent_types::RepoRoot). This crate does not run git, does
//! not open network connections, and does not log credential values. Errors
//! are [`JevError`](sc_agent_types::JevError) values and never include secret
//! material
//! (ADR-SAG-0003). Log lines stay under NFR-SAG-0002 and ADR-SAG-0013; this
//! crate emits none.
//!
//! [`resolve_key`] returns a [`ResolvedApiKey`] built by
//! [`ResolvedApiKey::try_new`], so key normalization (trim, reject interior
//! whitespace and control characters) happens once, here. Its `Debug` and
//! `Display` implementations redact the secret (ADR-SAG-0020).
//!
//! [`EnvSource`] and [`KeyringSource`] are open injection traits. Callers
//! that need fakes implement them directly, as the example below does.
//!
//! The repo-root `.env` dialect (comments, quoting, `export`) is documented
//! in `docs/sc-agent-secrets/architecture.md`.
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
    OPENAI_API_KEY_VAR, TYPESAFE_API_KEY_VAR, resolve_key, resolve_key_with,
};
#[doc(inline)]
pub use sc_agent_types::ResolvedApiKey;

// Crate-root name so `cargo test -- test_env_dotenv_keychain_order --exact` selects it.
#[cfg(test)]
#[test]
fn test_env_dotenv_keychain_order() {
    use sc_agent_types::BackendKeyId;

    use crate::resolve::{MapEnv, MapKeyring};
    use crate::resolve_key_with;
    use crate::test_support::TempRepo;

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
    assert_eq!(
        from_env,
        ResolvedApiKey::try_new(BackendKeyId::TypeSafe, "from-env".to_owned()).unwrap()
    );

    let from_dotenv = resolve_key_with(
        BackendKeyId::Anthropic,
        &repo.root(),
        &env,
        &keyring,
        "tester",
    )
    .unwrap();
    assert_eq!(
        from_dotenv,
        ResolvedApiKey::try_new(BackendKeyId::TypeSafe, "from-dotenv-anthropic".to_owned())
            .unwrap()
    );

    let from_keyring =
        resolve_key_with(BackendKeyId::OpenAi, &repo.root(), &env, &keyring, "tester").unwrap();
    assert_eq!(
        from_keyring,
        ResolvedApiKey::try_new(BackendKeyId::TypeSafe, "from-keyring-openai".to_owned()).unwrap()
    );

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
