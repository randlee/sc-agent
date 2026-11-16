//! Test-only: real [`ApiKey`] values built through `resolve_var` over an
//! in-memory environment (no process environment, no keyring).

#![cfg(test)]

use sc_agent_secrets::{ApiKey, EnvSource, KeySources, KeyringSource, SecretsError, resolve_var};

struct OneVar(String);

impl EnvSource for OneVar {
    fn var(&self, name: &str) -> Option<String> {
        (name == "SC_AGENT_HTTP_TEST_KEY").then(|| self.0.clone())
    }
}

struct NoKeyring;

impl KeyringSource for NoKeyring {
    fn password(&self, _: &str, _: &str) -> Result<Option<String>, SecretsError> {
        Ok(None)
    }
}

/// The key `secret`, resolved as `sc_agent_secrets` resolves every key.
pub(crate) fn key(secret: &str) -> ApiKey {
    let var = "SC_AGENT_HTTP_TEST_KEY".parse().expect("var name");
    let sources = KeySources {
        env: &OneVar(secret.to_owned()),
        repo_root: None,
        keyring: &NoKeyring,
    };
    resolve_var(&var, &sources).expect("test key").key
}
