use sc_agent_config::AgentConfigError;
use sc_agent_types::TypesError;

#[test]
fn every_configuration_error_matches_its_own_catalog() {
    let samples = [
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
