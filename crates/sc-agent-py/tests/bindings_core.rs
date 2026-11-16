//! Rust-side binding logic without linking the PyO3 extension or libpython.
//! The same Python-free source module is used by the production binding.

#[path = "../src/keyring_options.rs"]
mod keyring_options;

use std::sync::{Arc, Mutex};

use decidr::input::build_request_bytes;
use keyring_options::location;
use sc_agent_secrets::{KeyringSource, SecretsError};
use sc_agent_types::Provider;
use sc_decision_schema::DecisionResult;

type Calls = Arc<Mutex<Vec<(String, String)>>>;
struct RecordingKeyring(Calls);

impl KeyringSource for RecordingKeyring {
    fn password(&self, service: &str, account: &str) -> Result<Option<String>, SecretsError> {
        self.0
            .lock()
            .map_err(|_| SecretsError::KeyringRead {
                kind: "test",
                cause: "recording lock poisoned",
                source: None,
            })?
            .push((service.to_owned(), account.to_owned()));
        Ok(None)
    }
}

/// REQ-SAG-0007: result serialization retains the exact probability.
#[test]
fn a_probability_is_returned_unchanged() -> Result<(), String> {
    let body = br#"{"model":"jev-latest","answers":{"q":{"type":"noul","noul":0.123456789}},"usage":{"input_tokens":1,"output_tokens":1}}"#;
    let response = sc_decision_schema::SystemOneResponse::from_json(200, body)
        .map_err(|error| error.to_string())?;
    let request = build_request_bytes(
        br#"{"model":"jev-latest","state":"s","questions":{"q":{"type":"noul"}}}"#,
        Vec::new(),
        false,
        None,
    )
    .map_err(|error| error.to_string())?;
    let result = DecisionResult::new(
        sc_agent_types::SessionId::new(),
        0,
        request.digests().clone(),
        Ok(response),
    );
    let text = decidr::output::result_text(&result).map_err(|error| error.to_string())?;
    assert!(text.contains("\"noul\":0.123456789"), "{text}");
    Ok(())
}

#[test]
fn invalid_keyring_overrides_are_rejected() {
    for (service, account, expected) in [
        (Some(""), None, "keyring_service"),
        (None, Some(""), "keyring_account"),
    ] {
        assert!(
            matches!(location(service, account), Err(SecretsError::InvalidLocation { option }) if option == expected)
        );
    }
    assert!(location(Some("consumer"), None).unwrap().is_some());
    assert!(location(None, None).unwrap().is_none());
}

#[test]
fn valid_keyring_overrides_use_the_injected_provider_scoped_backend() -> Result<(), String> {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let location = location(Some("service"), Some("account"))
        .map_err(|error| error.to_string())?
        .ok_or("missing location")?;
    let keyring = sc_agent_secrets::ProviderKeyring::new(
        Provider::TypeSafe,
        location,
        Arc::new(RecordingKeyring(calls.clone())),
    );
    keyring
        .password(sc_agent_secrets::TYPESAFE_API_KEY, "os-user")
        .map_err(|error| error.to_string())?;
    keyring
        .password(sc_agent_secrets::ANTHROPIC_API_KEY, "os-user")
        .map_err(|error| error.to_string())?;
    assert_eq!(
        *calls.lock().map_err(|error| error.to_string())?,
        [
            ("service".to_owned(), "account".to_owned()),
            (
                sc_agent_secrets::ANTHROPIC_API_KEY.to_owned(),
                "os-user".to_owned()
            ),
        ]
    );
    Ok(())
}
