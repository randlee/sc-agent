use sc_agent_config::AgentConfigError;
use sc_agent_types::TypesError;

#[test]
fn every_configuration_error_matches_its_own_catalog() {
    let samples = [
        AgentConfigError::ProjectRequired { cli: "decidr.json" },
        AgentConfigError::Layer {
            source: "a.json".into(),
            cause: Box::new(AgentConfigError::Invalid {
                path: "workers".into(),
                message: "bad".into(),
            }),
        },
        AgentConfigError::Read {
            file: "f".into(),
            cause: std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        },
        AgentConfigError::Json {
            file: "f".into(),
            cause: serde_json::from_str::<serde_json::Value>("{").unwrap_err(),
        },
        AgentConfigError::Load(sc_config::ConfigError::MissingDefault {
            path: "missing".into(),
        }),
        AgentConfigError::Catalog(TypesError::UnknownProvider { text: "x".into() }),
        AgentConfigError::HomeUnknown,
        AgentConfigError::FileTooLarge {
            file: "f".into(),
            bytes: 1,
        },
        AgentConfigError::NotAFile { file: "f".into() },
        AgentConfigError::RunnerName {
            name: "n".into(),
            path: "p".into(),
        },
        AgentConfigError::Invalid {
            path: "p".into(),
            message: "m".into(),
        },
        AgentConfigError::KeyOnPlainHttp {
            path: "p".into(),
            host: "h".into(),
        },
    ];
    sc_agent_testkit::assert_catalog_rows(&samples);
}
