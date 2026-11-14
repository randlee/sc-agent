//! Checked, provider-scoped keyring location overrides.

use std::sync::Arc;

use sc_agent_types::Provider;

use crate::{KeyringSource, SecretsError, env_var};

/// One optional keyring service/account pair. Each provided component is 1..=255 UTF-8 bytes.
#[derive(Debug, Clone, Default)]
pub struct KeyringLocation {
    service: Option<String>,
    account: Option<String>,
}

impl KeyringLocation {
    /// Checks both components before any credential store or network operation.
    ///
    /// # Errors
    /// Returns `secrets.invalid_location` naming the option and bound, without its value.
    pub fn new(service: Option<&str>, account: Option<&str>) -> Result<Self, SecretsError> {
        fn checked(
            option: &'static str,
            value: Option<&str>,
        ) -> Result<Option<String>, SecretsError> {
            match value {
                Some(value) if !(1..=255).contains(&value.len()) => {
                    Err(SecretsError::InvalidLocation { option })
                }
                value => Ok(value.map(str::to_owned)),
            }
        }
        Ok(Self {
            service: checked("keyring_service", service)?,
            account: checked("keyring_account", account)?,
        })
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
                self.location.service.as_deref().unwrap_or(service),
                self.location.account.as_deref().unwrap_or(account),
            )
        } else {
            self.source.password(service, account)
        }
    }
}
