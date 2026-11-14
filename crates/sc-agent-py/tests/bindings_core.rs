//! Rust-side binding logic without linking the PyO3 extension or libpython.
//! The same Python-free source module is used by the production binding.

#[path = "../src/keyring_options.rs"]
mod keyring_options;

use std::cell::Cell;
use std::sync::{Arc, Mutex};

use decidr::input::build_request_bytes;
use keyring_options::{KeyringOverrideError, with_factory};
use sc_agent_secrets::{KeyringSource, SecretsError};
use sc_agent_types::{ErrorId, Provider};
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
fn invalid_keyring_overrides_do_not_construct_a_backend() {
    for (provider, service, account, expected) in [
        (Some("typesafe"), Some(""), None, "keyring_service"),
        (None, Some("consumer"), None, "provider"),
        (
            Some("not-a-provider"),
            Some("consumer"),
            None,
            "types.unknown_provider",
        ),
    ] {
        let constructed = Cell::new(false);
        let result = with_factory(provider, service, account, |_, _| {
            constructed.set(true);
            Arc::new(RecordingKeyring(Arc::new(Mutex::new(Vec::new()))))
        });
        assert!(
            !constructed.get(),
            "backend factory ran for a rejected override"
        );
        match result {
            Err(KeyringOverrideError::InvalidOption { option, bound }) => {
                assert_eq!(option, expected);
                assert!(!bound.is_empty());
            }
            Err(KeyringOverrideError::Provider(error)) => assert_eq!(error.id(), expected),
            Ok(_) => panic!("invalid override unexpectedly accepted"),
        }
    }
}

#[test]
fn valid_keyring_overrides_use_the_injected_provider_scoped_backend() -> Result<(), String> {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let keyring = with_factory(
        Some("typesafe"),
        Some("service"),
        Some("account"),
        |provider, location| {
            assert_eq!(provider, Provider::TypeSafe);
            Arc::new(sc_agent_secrets::ProviderKeyring::new(
                provider,
                location,
                Arc::new(RecordingKeyring(calls.clone())),
            ))
        },
    )
    .map_err(|error| format!("{error:?}"))?
    .ok_or("expected keyring override")?;
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
