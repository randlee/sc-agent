//! No platform store or network: a recording keyring builder refuses each lookup.
use keyring::credential::{Credential, CredentialBuilderApi};
use std::sync::{Arc, Mutex};

struct Recording(Arc<Mutex<Vec<(String, String)>>>);
impl CredentialBuilderApi for Recording {
    fn build(
        &self,
        _: Option<&str>,
        service: &str,
        user: &str,
    ) -> keyring::Result<Box<Credential>> {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push((service.into(), user.into()));
        Err(keyring::Error::NoEntry)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn keyring_overrides_reach_resolution_and_do_not_persist() {
    // Isolate process-global keyring configuration and inherited credential variables.
    if std::env::var_os("SC_KEYRING_TEST_CHILD").is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .env_clear()
            .env("SC_KEYRING_TEST_CHILD", "1")
            .args([
                "--exact",
                "keyring_overrides_reach_resolution_and_do_not_persist",
                "--nocapture",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let seen = Arc::new(Mutex::new(Vec::new()));
    keyring::set_default_credential_builder(Box::new(Recording(seen.clone())));
    let repo = tempfile::tempdir().unwrap();
    for location in [
        None,
        Some((Some("custom"), None)),
        Some((None, Some("alice"))),
        Some((Some("both"), Some("bob"))),
        None,
    ] {
        assert!(
            decidr::open(
                "haiku",
                Some("anthropic"),
                Some(repo.path()),
                sc_agent_http::Policy::default(),
                None,
                location
            )
            .is_err()
        );
    }
    let seen = seen.lock().unwrap();
    let var = sc_agent_types::providers::Provider::by_name("anthropic")
        .unwrap()
        .key_var;
    assert_eq!(seen.len(), 5);
    assert_eq!(seen[0].0, var);
    assert!(!seen[0].1.is_empty());
    assert_eq!(seen[1], ("custom".into(), seen[0].1.clone()));
    assert_eq!(seen[2], (var.into(), "alice".into()));
    assert_eq!(seen[3], ("both".into(), "bob".into()));
    assert_eq!(seen[4], seen[0]);
}
