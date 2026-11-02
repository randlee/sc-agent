//! Process environment and platform keyring adapters.
//!
//! Keyring failures become [`JevError::ConfigInvalid`] with a fixed detail
//! string. Upstream messages are dropped because `BadEncoding` carries the
//! raw secret bytes.

use sc_agent_types::JevError;

use crate::resolve::{EnvSource, KeyringSource};

pub(crate) const KEYRING_UNREADABLE: &str = "platform keyring could not be read";

/// Reads variables from the process environment.
///
/// A missing variable is `None`. A present empty string is `Some("")`, so
/// repo-root `.env` does not override it. Non-Unicode values are treated as
/// present and empty.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProcessEnv;

impl EnvSource for ProcessEnv {
    fn var(&self, name: &str) -> Option<String> {
        match std::env::var(name) {
            Ok(value) => Some(value),
            Err(std::env::VarError::NotPresent) => None,
            Err(std::env::VarError::NotUnicode(_)) => Some(String::new()),
        }
    }
}

/// Reads secrets from the platform keyring.
///
/// On macOS and iOS this is the Keychain. On Windows it is Credential Manager.
/// On Linux and BSD it is the Secret Service. Other targets have no keystore
/// and lookups miss.
///
/// # Panics
///
/// Panics if the process-global keyring builder lock is poisoned. That poison
/// means another thread already panicked while building an entry.
#[derive(Debug, Default, Clone, Copy)]
pub struct PlatformKeyring;

impl KeyringSource for PlatformKeyring {
    fn password(&self, service: &str, account: &str) -> Result<Option<String>, JevError> {
        let entry = match keyring::Entry::new(service, account) {
            Ok(entry) => entry,
            Err(_) => return Err(keyring_unavailable()),
        };
        classify_keyring(entry.get_password())
    }
}

pub(crate) fn classify_keyring(
    result: Result<String, keyring::Error>,
) -> Result<Option<String>, JevError> {
    match result {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(keyring_unavailable()),
    }
}

fn keyring_unavailable() -> JevError {
    JevError::ConfigInvalid {
        detail: KEYRING_UNREADABLE.to_owned(),
    }
}

/// OS user name used as the keyring account.
#[must_use]
pub(crate) fn current_account() -> String {
    whoami::username()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyring_errors_drop_secret_bytes() {
        let secret = "super-secret-value";
        let err = classify_keyring(Err(keyring::Error::BadEncoding(secret.as_bytes().to_vec())))
            .expect_err("bad encoding");
        assert_eq!(err.code(), "CONFIG_INVALID");
        assert_secret_absent(&err, secret);

        let err = classify_keyring(Err(keyring::Error::PlatformFailure(Box::new(
            std::io::Error::other(secret),
        ))))
        .expect_err("platform failure");
        assert_secret_absent(&err, secret);
        assert_eq!(
            classify_keyring(Err(keyring::Error::NoEntry)).unwrap(),
            None
        );
    }

    fn assert_secret_absent(error: &JevError, secret: &str) {
        let rendered = format!("{error} {error:?} {}", error.fix_hint());
        assert!(!rendered.contains(secret), "{rendered}");
    }
}
