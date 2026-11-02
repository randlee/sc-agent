//! Process environment and platform keyring adapters.
//!
//! Keyring failures become [`JevError::ConfigInvalid`] with a sanitized cause
//! and a recovery hint. Upstream messages are dropped because `BadEncoding`
//! carries the raw secret bytes and platform errors can echo caller text.

use sc_agent_types::JevError;

use crate::resolve::{EnvSource, KeyringSource};

pub(crate) const KEYRING_UNREADABLE: &str = "platform keyring could not be read";

pub(crate) const KEYRING_RECOVERY: &str = "Unlock the platform keystore for this OS user and retry. Do not paste secret values into logs or this message.";

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
            Err(error) => return Err(keyring_unavailable(sanitized_keyring_cause(&error))),
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
        Err(error) => Err(keyring_unavailable(sanitized_keyring_cause(&error))),
    }
}

/// Labels a keyring failure without formatting the error.
///
/// `keyring::Error`'s `Display` and `Debug` can include secret bytes
/// (`BadEncoding`) or the inner platform message (`PlatformFailure`).
fn sanitized_keyring_cause(error: &keyring::Error) -> &'static str {
    match error {
        keyring::Error::PlatformFailure(_) => "platform keystore returned an error",
        keyring::Error::NoStorageAccess(_) => "keystore access was denied",
        keyring::Error::NoEntry => "entry is absent",
        keyring::Error::BadEncoding(_) => "stored value was not valid UTF-8",
        keyring::Error::TooLong(_, _) => "keystore rejected an attribute length",
        keyring::Error::Invalid(_, _) => "keystore rejected the service or account",
        keyring::Error::Ambiguous(_) => "keystore matched more than one entry",
        _ => "keystore returned an unrecognized error",
    }
}

fn keyring_unavailable(cause: &str) -> JevError {
    JevError::credential_store_invalid(KEYRING_UNREADABLE, cause, KEYRING_RECOVERY)
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
        let cases = [
            (
                keyring::Error::BadEncoding(secret.as_bytes().to_vec()),
                "stored value was not valid UTF-8",
            ),
            (
                keyring::Error::PlatformFailure(Box::new(std::io::Error::other(secret))),
                "platform keystore returned an error",
            ),
            (
                keyring::Error::NoStorageAccess(Box::new(std::io::Error::other(secret))),
                "keystore access was denied",
            ),
            (
                keyring::Error::TooLong(secret.to_owned(), 8),
                "keystore rejected an attribute length",
            ),
            (
                keyring::Error::Invalid("password".to_owned(), secret.to_owned()),
                "keystore rejected the service or account",
            ),
        ];
        for (error, cause) in cases {
            let err = classify_keyring(Err(error)).expect_err("keyring failure");
            assert_eq!(err.code(), "CONFIG_INVALID");
            let hint = err.fix_hint();
            assert!(hint.contains(KEYRING_UNREADABLE), "{hint}");
            assert!(hint.contains(cause), "{hint}");
            assert!(hint.contains(KEYRING_RECOVERY), "{hint}");
            assert!(!hint.contains("check.toml"), "{hint}");
            assert_secret_absent(&err, secret);
        }
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
