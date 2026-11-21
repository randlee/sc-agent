use sc_agent_log::LogError;
use sc_decision_runner::RunnerError;

#[test]
fn every_runner_error_matches_its_own_catalog() {
    let samples = [
        RunnerError::SessionLog(LogError::InvalidSlug {
            field: "project",
            value: "bad".into(),
        }),
        RunnerError::InvalidConfig {
            workers: 0,
            max: 64,
        },
    ];
    sc_agent_testkit::assert_catalog_rows(&samples);
}
