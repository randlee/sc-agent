//! Python-free validation and construction of provider-scoped keyring overrides.

use std::sync::Arc;

use sc_agent_secrets::{
    KeyringAccount, KeyringLocation, KeyringService, KeyringSource, SecretsError,
};
use sc_agent_types::{Provider, TypesError};

pub(crate) type KeyringBackend = Arc<dyn KeyringSource + Send + Sync>;
pub(crate) type PreparedKeyring = (Option<Provider>, Option<KeyringBackend>);

#[derive(Debug)]
pub(crate) enum KeyringOverrideError {
    InvalidOption {
        option: &'static str,
        bound: &'static str,
    },
    Provider(TypesError),
}

/// Validate every option before invoking the injected backend constructor.
pub(crate) fn with_factory(
    provider: Option<&str>,
    service: Option<&str>,
    account: Option<&str>,
    make_backend: impl FnOnce(Provider, KeyringLocation) -> KeyringBackend,
) -> Result<PreparedKeyring, KeyringOverrideError> {
    let service = service
        .map(KeyringService::new)
        .transpose()
        .map_err(|error| {
            let option = match error {
                SecretsError::InvalidLocation { option } => option,
                _ => "keyring",
            };
            KeyringOverrideError::InvalidOption {
                option,
                bound: "must contain 1..=255 UTF-8 bytes",
            }
        })?;
    let account = account
        .map(KeyringAccount::new)
        .transpose()
        .map_err(|error| {
            let option = match error {
                SecretsError::InvalidLocation { option } => option,
                _ => "keyring",
            };
            KeyringOverrideError::InvalidOption {
                option,
                bound: "must contain 1..=255 UTF-8 bytes",
            }
        })?;
    let has_override = service.is_some() || account.is_some();
    if !has_override {
        return Ok((None, None));
    }
    let provider = provider
        .map(str::parse::<Provider>)
        .transpose()
        .map_err(KeyringOverrideError::Provider)?;
    let provider = provider.ok_or(KeyringOverrideError::InvalidOption {
        option: "provider",
        bound: "is required with a keyring override",
    })?;
    let location = KeyringLocation::new(service, account);
    Ok((Some(provider), Some(make_backend(provider, location))))
}
