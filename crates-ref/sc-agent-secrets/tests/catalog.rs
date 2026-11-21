use sc_agent_secrets::SecretsError;

#[test]
fn every_secrets_error_matches_its_own_catalog() {
    let samples = [
        SecretsError::InvalidLocation { option: "service" },
        SecretsError::KeyMissing { var: "V".into() },
        SecretsError::KeyMalformed {
            var: "V".into(),
            reason: "invalid",
        },
        SecretsError::DotenvRead {
            kind: "io".into(),
            cause: "read",
            source: None,
        },
        SecretsError::KeyringRead {
            kind: "io",
            cause: "read",
            source: None,
        },
        SecretsError::KeyringTimeout,
        SecretsError::KeyringConstructionRefused,
    ];
    sc_agent_testkit::assert_catalog_rows(&samples);
}
