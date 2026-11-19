//! This integration-test binary owns its interpreter: no shared import-state mutation.
use pyo3::prelude::*;
use pyo3::types::PyModule;
use sc_agent_log::{GeneralLogger, Level, LogContext, LogError};
use sc_agent_types::{ErrorCategory, ErrorId};
use serde_json::Map;

#[test]
fn callable_without_package_and_construction_failures_are_distinct_from_admission() {
    Python::attach(|py| {
        let module = PyModule::from_code(
            py,
            c"
import sys
import types
sys.modules['sc_observability'] = None
records = []
def callback(record): records.append(record)
def fail(record): raise ValueError('controlled failure without package')
",
            c"without_observability.py",
            c"without_observability",
        )
        .unwrap();
        let callable =
            GeneralLogger::injected_python(module.getattr("callback").unwrap().unbind(), "decidr")
                .unwrap();
        callable
            .write(
                Level::Info,
                "test",
                "test.row",
                LogContext::NONE,
                Map::new(),
            )
            .unwrap();
        callable.close().unwrap();
        assert_eq!(module.getattr("records").unwrap().len().unwrap(), 1);

        let unavailable = GeneralLogger::injected_python(py.None(), "decidr").unwrap_err();
        assert!(matches!(unavailable, LogError::PythonUnavailable { .. }));
        assert_eq!(unavailable.id(), "log.python_unavailable");
        assert_eq!(unavailable.detail().category, Some(ErrorCategory::Log));
        assert!(unavailable.message().contains("install sc-observability"));
        let cause = unavailable.cause_detail().unwrap();
        assert!(cause.message.contains("ModuleNotFoundError"));
        assert!(cause.message.contains("sc_observability"));

        let raising =
            GeneralLogger::injected_python(module.getattr("fail").unwrap().unbind(), "decidr")
                .unwrap();
        let rejected = raising
            .write(
                Level::Info,
                "test",
                "test.row",
                LogContext::NONE,
                Map::new(),
            )
            .unwrap_err();
        assert!(matches!(rejected, LogError::PythonRejected { .. }));
        assert_eq!(rejected.id(), "log.python_rejected");
        assert_eq!(
            rejected.cause_detail().unwrap().message,
            "ValueError: controlled failure without package"
        );
        let LogError::RowsLost {
            count,
            last: Some(last),
            ..
        } = raising.flush().unwrap_err()
        else {
            panic!("expected one lost row")
        };
        assert_eq!(count, 1);
        assert_eq!(Some(*last), rejected.cause_detail());
        raising.flush().unwrap();

        // A present package with an unusable type surface fails at classification.
        py.import("sys")
            .unwrap()
            .getattr("modules")
            .unwrap()
            .set_item(
                "sc_observability",
                PyModule::new(py, "sc_observability").unwrap(),
            )
            .unwrap();
        let classify = GeneralLogger::injected_python(py.None(), "decidr").unwrap_err();
        assert!(matches!(classify, LogError::PythonClassify { .. }));
        assert_eq!(classify.id(), "log.python_classify");
        assert_eq!(classify.detail().category, Some(ErrorCategory::Log));
        assert!(
            classify
                .cause_detail()
                .unwrap()
                .message
                .contains("AttributeError")
        );
    });
}
