#![forbid(unsafe_code)]
//! decidr's composition root: picks the provider and key, builds the backend once, and
//! returns a [`Decision`]. The CLI and the Python module both start here.

use sc_agent_http::Policy;
use sc_agent_log::Session;
use sc_agent_types::Error;
use sc_decision_runner::{Decider, Decision};
use sc_decision_schema::Backend;

/// The decision backend for a provider: the Jev API itself, or a chat model through
/// [`FromLlm`]. The key and base URL are checked here, once.
pub fn backend(
    provider: &sc_agent_types::providers::Provider,
    key: Option<String>,
) -> Result<Box<dyn Backend>, Error> {
    match provider.wire {
        sc_agent_types::providers::Wire::SystemOne => Ok(Box::new(
            sc_decision_systemone::SystemOne::new(provider.base_url, key.as_deref())?,
        )),
        _ => Ok(Box::new(sc_decision_llm::FromLlm::new(
            sc_llm_provider::backend(provider, key)?,
        ))),
    }
}

/// A decider for `model` (short name or id) on `provider`, or the first provider in ranking
/// order with a key. Returns it with the provider's model id, which goes into each request.
pub fn open(
    model: &str,
    provider: Option<&str>,
    repo_root: Option<&std::path::Path>,
    policy: Policy,
    session: Option<Session>,
) -> Result<(Box<dyn Decision>, String), Error> {
    open_with_keyring(model, provider, repo_root, policy, session, None)
}

/// Like open, with an explicitly supplied keyring location for the selected provider only.
pub fn open_with_keyring(
    model: &str,
    provider: Option<&str>,
    repo_root: Option<&std::path::Path>,
    policy: Policy,
    session: Option<Session>,
    keyring: Option<(Option<&str>, Option<&str>)>,
) -> Result<(Box<dyn Decision>, String), Error> {
    let provider = if keyring.is_some() && provider.is_none() {
        let id = sc_agent_types::providers::model_id(model);
        let ranked: Vec<_> = sc_agent_types::providers::PROVIDERS
            .iter()
            .filter(|p| !p.serves.is_empty() && p.serves(id))
            .collect();
        match ranked.as_slice() {
            [one] => Some(one.name),
            _ => {
                return Err(Error::AmbiguousKeyringOverride(
                    ranked.iter().map(|p| p.name).collect::<Vec<_>>().join(", "),
                ));
            }
        }
    } else {
        provider
    };
    let key = |p: &sc_agent_types::providers::Provider| {
        sc_agent_secrets::key_with_location(p.key_var, repo_root, keyring)
    };
    let (chosen, id) = sc_agent_types::providers::Provider::choose(model, provider, |p| {
        matches!(key(p), Ok(Some(_)))
    })?;
    let found = key(chosen)?.map(|(value, _)| value);
    let found = Some(found.ok_or_else(|| Error::MissingKey {
        provider: chosen.name.into(),
        var: chosen.key_var.into(),
    })?);
    Ok((
        Box::new(Decider::new(backend(chosen, found)?, policy, session)),
        id,
    ))
}
