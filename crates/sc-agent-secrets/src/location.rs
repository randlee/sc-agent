//! Checked, provider-scoped keyring location overrides.

use std::sync::Arc;

use sc_agent_types::Provider;

use crate::{KeyringSource, SecretsError, env_var};

/// A checked keyring service name (1..=255 UTF-8 bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyringService(String);

impl KeyringService {
    /// Checks the service name before any credential store or network operation.
    ///
    /// # Errors
    /// Returns `secrets.invalid_location` naming the option and bound, without its value.
    pub fn new(value: &str) -> Result<Self, SecretsError> {
        checked_location("keyring_service", value).map(Self)
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

/// A checked keyring account name (1..=255 UTF-8 bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyringAccount(String);

impl KeyringAccount {
    /// Checks the account name before any credential store or network operation.
    ///
    /// # Errors
    /// Returns `secrets.invalid_location` naming the option and bound, without its value.
    pub fn new(value: &str) -> Result<Self, SecretsError> {
        checked_location("keyring_account", value).map(Self)
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

fn checked_location(option: &'static str, value: &str) -> Result<String, SecretsError> {
    if !(1..=255).contains(&value.len()) {
        return Err(SecretsError::InvalidLocation { option });
    }
    Ok(value.to_owned())
}

/// One optional keyring service/account pair whose components were checked at construction.
#[derive(Debug, Clone, Default)]
pub struct KeyringLocation {
    service: Option<KeyringService>,
    account: Option<KeyringAccount>,
}

impl KeyringLocation {
    /// Stores independently checked components; omitted values retain lookup defaults.
    #[must_use]
    pub fn new(service: Option<KeyringService>, account: Option<KeyringAccount>) -> Self {
        Self { service, account }
    }
}

/// Applies a checked location to exactly one provider, leaving every other lookup unchanged.
pub struct ProviderKeyring {
    provider: Provider,
    location: KeyringLocation,
    source: Arc<dyn KeyringSource>,
}

impl ProviderKeyring {
    /// Wraps a source; omitted components independently retain their lookup defaults.
    #[must_use]
    pub fn new(
        provider: Provider,
        location: KeyringLocation,
        source: Arc<dyn KeyringSource>,
    ) -> Self {
        Self {
            provider,
            location,
            source,
        }
    }
}

impl KeyringSource for ProviderKeyring {
    fn password(&self, service: &str, account: &str) -> Result<Option<String>, SecretsError> {
        if service == env_var(self.provider) {
            self.source.password(
                self.location
                    .service
                    .as_ref()
                    .map_or(service, KeyringService::as_str),
                self.location
                    .account
                    .as_ref()
                    .map_or(account, KeyringAccount::as_str),
            )
        } else {
            self.source.password(service, account)
        }
    }
}
