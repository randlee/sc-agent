use sc_agent_secrets::{PlatformKeyring, SecretsError};
use sc_agent_types::ErrorId;

#[test]
fn test_dependency_build_guard_refuses_platform_backend() {
    let Err(error) = PlatformKeyring::new() else {
        panic!("test dependency must arm the platform keyring guard");
    };

    assert!(matches!(error, SecretsError::KeyringConstructionRefused));
    assert_eq!(error.id(), "secrets.keyring_construction_refused");
    assert_eq!(
        error.recovery(),
        Some(
            "use an injected keyring source in tests; do not access the platform keyring from a test"
        )
    );
}
