//! The binding's catalog, tested without loading the Python extension or keyring.
#[path = "../src/binding_error.rs"]
mod binding_error;

#[test]
fn every_python_binding_error_matches_its_own_catalog() {
    let samples = [
        binding_error::PyBindingError::WorkerPanicked {
            message: "failed".into(),
        },
        binding_error::PyBindingError::InvalidOption {
            option: "x".into(),
            bound: "bad".into(),
        },
    ];
    sc_agent_testkit::assert_catalog_rows(&samples);
}
