//! Requires sc-observability 1.5.0 importable by the embedded Python interpreter.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};
use sc_agent_log::{
    GeneralLogger, Injected, Level, LogContext, LogError, Loggers, Repo, SessionIdentity,
};
use sc_agent_types::{ErrorId, SessionId};
use serde_json::Map;
use std::ffi::CString;

fn module(root: &std::path::Path, name: &std::ffi::CStr) -> Py<PyModule> {
    Python::attach(|py| {
        let source = CString::new(format!(
            r#"
import sc_observability as obs
root = {root:?}
logger = obs.create_logger(obs.LoggerConfig(service='injected', log_root=root)).value
attached = obs.AttachedLogger(logger._native)
records = []
def callback(record): records.append(record)
def fail(record): raise ValueError('controlled failure')
"#,
            root = root.to_string_lossy()
        ))
        .unwrap();
        PyModule::from_code(py, &source, c"injected.py", name)
            .unwrap()
            .unbind()
    })
}
fn target(module: &Py<PyModule>, name: &str) -> Py<PyAny> {
    Python::attach(|py| module.getattr(py, name).unwrap())
}
fn write(logger: &GeneralLogger) -> Result<(), LogError> {
    logger.write(
        Level::Info,
        "test",
        "test.row",
        LogContext::NONE,
        Map::new(),
    )
}
fn identity() -> SessionIdentity {
    SessionIdentity::new(
        Repo::from_origin_url("https://github.com/randlee/sc-agent.git").unwrap(),
        "phase-b",
        "rust-developer",
    )
    .unwrap()
}
#[test]
fn real_logger_attached_logger_callable_and_failures() {
    let root = tempfile::tempdir().unwrap();
    let python_root = root.path().join("python");
    let module = module(&python_root, c"existing_injection");
    for name in ["logger", "attached"] {
        let log = GeneralLogger::injected_python(target(&module, name), "decidr").unwrap();
        write(&log).unwrap();
        log.close().unwrap();
    }
    Python::attach(|py| {
        let logger = module.getattr(py, "logger").unwrap();
        assert_eq!(
            logger
                .call_method0(py, "flush")
                .unwrap()
                .getattr(py, "kind")
                .unwrap()
                .extract::<String>(py)
                .unwrap(),
            "ok"
        );
    });
    let rows = std::fs::read_to_string(python_root.join("logs/injected.log.jsonl")).unwrap();
    assert_eq!(rows.lines().count(), 2);
    assert!(!root.path().join("logs/decidr.log.jsonl").exists());
    let both = Loggers::open_with(
        root.path(),
        "decidr",
        identity(),
        None,
        Some(Injected::Python(std::sync::Arc::new(target(
            &module, "callback",
        )))),
    )
    .unwrap();
    let session_id = SessionId::new();
    both.session.write(session_id, Map::new()).unwrap();
    both.general
        .write(
            Level::Info,
            "test",
            "test.row",
            LogContext::NONE,
            Map::new(),
        )
        .unwrap();
    both.close().unwrap();
    assert!(root.path().join("logs/decidr.log.jsonl").is_file());
    Python::attach(|py| {
        let records = module.getattr(py, "records").unwrap();
        assert_eq!(records.bind(py).len().unwrap(), 1);
        let row = records.bind(py).get_item(0).unwrap();
        let row = row.cast::<PyDict>().unwrap();
        let mut keys: Vec<String> = row
            .keys()
            .iter()
            .map(|key| key.extract().unwrap())
            .collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "action",
                "correlation_id",
                "fields",
                "level",
                "message",
                "outcome",
                "request_id",
                "target",
                "trace"
            ]
        );
        assert_eq!(
            row.get_item("correlation_id")
                .unwrap()
                .unwrap()
                .extract::<String>()
                .unwrap(),
            session_id.to_string()
        );
        for key in ["message", "outcome", "request_id", "trace"] {
            assert!(row.get_item(key).unwrap().unwrap().is_none(), "{key}");
        }
        assert_eq!(
            row.get_item("level")
                .unwrap()
                .unwrap()
                .extract::<String>()
                .unwrap(),
            "info"
        );
        assert!(
            row.get_item("fields")
                .unwrap()
                .unwrap()
                .cast::<PyDict>()
                .is_ok()
        );
    });
    let bad = Python::attach(|py| py.None());
    let error = GeneralLogger::injected_python(bad, "decidr").unwrap_err();
    assert_eq!(error.id(), "log.invalid_target");
    assert!(error.message().contains("AttachedLogger"));
    let raising = GeneralLogger::injected_python(target(&module, "fail"), "decidr").unwrap();
    assert!(write(&raising).is_err());
    assert!(matches!(
        raising.flush(),
        Err(LogError::RowsLost { count: 1, .. })
    ));
    Python::attach(|py| {
        module
            .getattr(py, "logger")
            .unwrap()
            .call_method0(py, "shutdown")
            .unwrap();
    });
    let stopped = GeneralLogger::injected_python(target(&module, "logger"), "decidr").unwrap();
    let error = write(&stopped).unwrap_err();
    let cause = error.cause_detail().unwrap();
    assert!(!cause.id.is_empty() && !cause.message.is_empty());
    let LogError::RowsLost {
        count,
        last: Some(last),
        ..
    } = stopped.flush().unwrap_err()
    else {
        panic!("expected refusal")
    };
    assert_eq!(count, 1);
    assert_eq!(last.id, cause.id);
    assert_eq!(last.message, cause.message);
    stopped.flush().unwrap();
}

#[test]
fn cloned_python_injection_opens_each_logger_independently() {
    let root = tempfile::tempdir().unwrap();
    let module = module(root.path(), c"independent_injection");
    let injected = Injected::Python(std::sync::Arc::new(target(&module, "callback")));
    // Clone outside Python::attach: the Rust owner does not require interpreter access.
    let second = injected.clone();
    let general = GeneralLogger::open_with(root.path(), "decidr", Some(injected)).unwrap();
    let session =
        sc_agent_log::SessionLogger::open_with(root.path(), identity(), Some(second)).unwrap();
    write(&general).unwrap();
    session.write(SessionId::new(), Map::new()).unwrap();
    general.close().unwrap();
    session.close().unwrap();
    Python::attach(|py| {
        let records = module.getattr(py, "records").unwrap();
        assert_eq!(records.bind(py).len().unwrap(), 2);
        module
            .getattr(py, "logger")
            .unwrap()
            .call_method0(py, "shutdown")
            .unwrap();
    });
    assert!(!root.path().join("logs/decidr.log.jsonl").exists());
    assert!(!root.path().join("sessions").exists());
}
