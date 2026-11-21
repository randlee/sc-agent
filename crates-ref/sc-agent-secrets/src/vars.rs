//! Credential environment-variable names (REQ-SAG-0401).
//!
//! These constants are the only credential-name string literals in the
//! workspace; `tests/vars_gate.rs` fails on any other occurrence. Each name is
//! both the environment variable and the platform keyring service.

use sc_agent_types::Provider;

/// Environment variable and default keyring service for TypeSafe.
pub const TYPESAFE_API_KEY: &str = "TYPESAFE_API_KEY";

/// Environment variable and default keyring service for Anthropic.
pub const ANTHROPIC_API_KEY: &str = "ANTHROPIC_API_KEY";

/// Environment variable and default keyring service for OpenAI.
pub const OPENAI_API_KEY: &str = "OPENAI_API_KEY";

/// Environment variable and keyring service for OpenRouter.
pub const OPENROUTER_API_KEY: &str = "OPENROUTER_API_KEY";

/// Environment variable and keyring service for Together.
pub const TOGETHER_API_KEY: &str = "TOGETHER_API_KEY";

/// Environment variable and keyring service for Nous.
pub const NOUS_API_KEY: &str = "NOUS_API_KEY";

/// Environment variable and keyring service for the Laya local runner.
///
/// Laya reads this variable itself (`laya/serve.py`); a local runner points at
/// it through its `key_env` setting and [`resolve_var`](crate::resolve_var).
pub const LAYA_API_KEY: &str = "LAYA_API_KEY";

/// Every credential variable name this crate knows, providers first.
pub const ALL_VARS: &[&str] = &[
    TYPESAFE_API_KEY,
    ANTHROPIC_API_KEY,
    OPENAI_API_KEY,
    OPENROUTER_API_KEY,
    TOGETHER_API_KEY,
    NOUS_API_KEY,
    LAYA_API_KEY,
];

/// Returns the credential variable name of `provider`.
///
/// # Examples
///
/// ```
/// use sc_agent_secrets::{env_var, OPENAI_API_KEY};
/// use sc_agent_types::Provider;
///
/// assert_eq!(env_var(Provider::OpenAi), OPENAI_API_KEY);
/// ```
#[must_use]
pub const fn env_var(provider: Provider) -> &'static str {
    match provider {
        Provider::TypeSafe => TYPESAFE_API_KEY,
        Provider::Anthropic => ANTHROPIC_API_KEY,
        Provider::OpenAi => OPENAI_API_KEY,
        Provider::OpenRouter => OPENROUTER_API_KEY,
        Provider::Nous => NOUS_API_KEY,
        Provider::Together => TOGETHER_API_KEY,
    }
}
