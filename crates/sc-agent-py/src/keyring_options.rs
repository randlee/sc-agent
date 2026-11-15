//! Python-free validation of keyring location overrides.

use sc_agent_secrets::{KeyringAccount, KeyringLocation, KeyringService, SecretsError};

pub(crate) fn location(
    service: Option<&str>,
    account: Option<&str>,
) -> Result<Option<KeyringLocation>, SecretsError> {
    let service = service.map(KeyringService::new).transpose()?;
    let account = account.map(KeyringAccount::new).transpose()?;
    if service.is_none() && account.is_none() {
        return Ok(None);
    }
    Ok(Some(KeyringLocation::new(service, account)))
}
