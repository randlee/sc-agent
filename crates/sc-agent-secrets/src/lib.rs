//! API key resolution for sc-agent (REQ-SAG-0401).
//!
//! A key is looked up by its variable name in the repo-root `.env` file, then
//! in the process environment, then in the platform keyring (service =
//! variable name, account = OS user). [`resolve`] takes a
//! [`Provider`](sc_agent_types::Provider) and uses [`env_var`]; [`resolve_var`]
//! takes a variable named by configuration (a local runner's `key_env`).
//!
//! The result is a sealed [`ApiKey`] whose `Debug` and `Display` print
//! `[redacted]`. Only sc-agent-http's transport reads it with
//! [`ApiKey::as_str`]; the workspace `clippy.toml` disallows every other call.
//! This crate logs nothing and never puts key bytes in an error.
//!
//! The constants in this crate are the only credential-name string literals in
//! the workspace (`tests/vars_gate.rs`).
#![forbid(unsafe_code)]

mod api_key;
mod dotenv;
mod env;
mod error;
mod keyring;
mod location;
mod resolve;
mod vars;

#[doc(inline)]
pub use api_key::ApiKey;
#[doc(inline)]
pub use env::{EnvSource, ProcessEnv};
#[doc(inline)]
pub use error::SecretsError;
#[doc(inline)]
pub use keyring::{KeyringSource, PlatformKeyring};
#[doc(inline)]
pub use resolve::{KeySource, KeySources, ResolvedKey, resolve, resolve_var};
#[doc(inline)]
pub use vars::{
    ALL_VARS, ANTHROPIC_API_KEY, LAYA_API_KEY, NOUS_API_KEY, OPENAI_API_KEY, OPENROUTER_API_KEY,
    TOGETHER_API_KEY, TYPESAFE_API_KEY, env_var,
};

#[doc(inline)]
pub use location::{KeyringLocation, ProviderKeyring};
