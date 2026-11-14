//! Python-free validation and construction of provider-scoped keyring overrides.

use std::sync::Arc;

use sc_agent_secrets::{KeyringLocation, KeyringSource, SecretsError};
use sc_agent_types::{Provider, TypesError};

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
    make_backend: impl FnOnce(Provider, KeyringLocation) -> Arc<dyn KeyringSource + Send + Sync>,
) -> Result<Option<Arc<dyn KeyringSource + Send + Sync>>, KeyringOverrideError> {
    let location = KeyringLocation::new(service, account).map_err(|error| {
        let option = match error {
            SecretsError::InvalidLocation { option } => option,
            _ => "keyring",
        };
        KeyringOverrideError::InvalidOption {
            option,
            bound: "must contain 1..=255 UTF-8 bytes",
        }
    })?;
    if service.is_none() && account.is_none() {
        return Ok(None);
    }
    let provider = provider.ok_or(KeyringOverrideError::InvalidOption {
        option: "provider",
        bound: "is required with a keyring override",
    })?;
    let provider = provider
        .parse::<Provider>()
        .map_err(KeyringOverrideError::Provider)?;
    Ok(Some(make_backend(provider, location)))
}
