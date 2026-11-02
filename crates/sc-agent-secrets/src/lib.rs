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
//! (ADR-SAG-0003).
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

#[doc(inline)]
pub use platform::{PlatformKeyring, ProcessEnv};
#[doc(inline)]
pub use resolve::{
    ANTHROPIC_API_KEY_VAR, ANTHROPIC_KEYCHAIN_SERVICE_VAR, EnvSource, KeyringSource,
    OPENAI_API_KEY_VAR, TYPESAFE_API_KEY_VAR, resolve_key, resolve_key_with,
};

#[cfg(any(test, feature = "test-util"))]
#[doc(inline)]
pub use resolve::{MapEnv, MapKeyring};
