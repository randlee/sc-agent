//! The Python module. Every call returns `{"ok": true, "value": ...}` or
//! `{"ok": false, "error": {"id", "message"}}` as JSON text; nothing raises.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::Arc;

use pyo3::prelude::*;
use sc_agent_http::{Cancel, Policy};
use sc_agent_log::{Session, record, repo_id};
use sc_agent_types::{Error, ErrorId};
use sc_decision_schema::DecisionRequest;
use serde_json::{Map, Value, json};

fn error_value(e: &Error) -> Value {
    json!({ "ok": false, "error": e.detail() })
}

fn reply(result: Result<Value, Error>) -> Value {
    match result {
        Ok(value) => json!({ "ok": true, "value": value }),
        Err(e) => error_value(&e),
    }
}

/// Runs `f` with the GIL released; a panic becomes an error value.
fn guarded<T: Send>(
    py: Python<'_>,
    f: impl FnOnce() -> Result<T, Error> + Send,
) -> Result<T, Error> {
    py.detach(|| catch_unwind(AssertUnwindSafe(f)))
        .unwrap_or_else(|_| Err(Error::Invalid("internal error (panic)".into())))
}

/// The general logger: obtained from this module, passed back into every decidr and sitr call,
/// with the log root it was opened under (sessions are written beside it, ADR-SAG-0021).
#[pyclass(frozen)]
struct Logger(Arc<dyn sc_agent_log::RecordSink>, PathBuf);

/// Cancels a running `send` or `send_batch`; unfinished requests come back cancelled.
#[pyclass(frozen)]
#[derive(Default)]
struct CancelToken(Cancel);

#[pymethods]
impl CancelToken {
    #[new]
    fn new() -> Self {
        Self::default()
    }
    fn cancel(&self) {
        self.0.cancel();
    }
    fn cancelled(&self) -> bool {
        self.0.is_cancelled()
    }
}

/// `(Logger, None)` writing to `<root>/logs/<name>.log.jsonl`, the root from
/// `sc_agent_log::log_root(name)` (`<NAME>_LOG_ROOT`, else `~/.sc-agent`), or `(None, error JSON)`.
#[pyfunction]
fn logger(name: &str) -> (Option<Logger>, Option<String>) {
    let opened = sc_agent_log::log_root(name)
        .and_then(|root| Ok(Logger(sc_agent_log::general(name, &root)?, root)));
    match opened {
        Ok(l) => (Some(l), None),
        Err(e) => (None, Some(error_value(&e).to_string())),
    }
}

/// Decides `requests` (JSON text each) and returns one reply per request, in input order.
#[expect(clippy::too_many_arguments, reason = "the Python keyword arguments")]
fn decide_all(
    py: Python<'_>,
    requests: Vec<String>,
    model: &str,
    project: &str,
    agent_role: &str,
    logger: &Logger,
    provider: Option<&str>,
    workers: usize,
    cancel: Option<&CancelToken>,
    session_log: Option<Py<PyAny>>,
    abort_on: Vec<String>,
    keyring_service: Option<String>,
    keyring_account: Option<String>,
) -> Vec<Value> {
    let count = requests.len();
    let cancel = cancel.map(|c| c.0.clone()).unwrap_or_default();
    let result = guarded(py, || {
        if workers == 0 {
            return Err(Error::Config("workers must be at least 1".into()));
        }
        if count == 0 {
            return Ok(Vec::new());
        }
        if cancel.is_cancelled() {
            return Err(Error::Cancelled { sends: 0 });
        }
        let requests = requests
            .iter()
            .enumerate()
            .map(|(i, r)| {
                serde_json::from_str::<DecisionRequest>(r)
                    .map_err(|e| Error::Config(format!("request {i}: {e}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let cwd = std::env::current_dir().unwrap_or_default();
        let session = match session_log {
            Some(sink) => Session::injected(
                sc_agent_log::python::PythonSink::open(sink)?,
                repo_id(&cwd),
                project,
                agent_role,
            )?,
            None => Session::open(&logger.1, repo_id(&cwd), project, agent_role)?,
        };
        let keyring = (keyring_service.is_some() || keyring_account.is_some())
            .then_some((keyring_service.as_deref(), keyring_account.as_deref()));
        let (decision, id) = decidr::open_with_keyring(
            model,
            provider,
            Some(&cwd),
            Policy::default(),
            Some(session),
            keyring,
        )?;
        let requests = requests
            .into_iter()
            .map(|r| DecisionRequest {
                model: id.clone(),
                ..r
            })
            .collect();
        let mut replies = vec![Value::Null; count];
        decision.decide_batch(requests, workers, &cancel, &mut |outcome| {
            if outcome
                .result
                .as_ref()
                .is_err_and(|e| abort_on.iter().any(|c| c == e.category()))
            {
                cancel.cancel();
            }
            let value = outcome.result.and_then(|r| {
                serde_json::to_value(r).map_err(|e| Error::ResponseParse {
                    message: e.to_string(),
                    status: None,
                })
            });
            if let Some(slot) = replies.get_mut(outcome.position) {
                *slot = reply(value);
            }
        });
        Ok(replies)
    });
    let replies = result.unwrap_or_else(|e| vec![error_value(&e); count]);
    for r in &replies {
        let mut fields = Map::new();
        fields.insert("model".into(), model.into());
        fields.insert("ok".into(), r.get("ok").cloned().unwrap_or_default());
        fields.insert(
            "error_id".into(),
            r.pointer("/error/id").cloned().unwrap_or_default(),
        );
        record(logger.0.as_ref(), "sc_agent.decidr", "decidr.send", fields);
    }
    logger.0.flush_records();
    replies
}

/// Decide one request (JSON text) with `model` on `provider` (or the first with a key).
#[pyfunction]
#[pyo3(signature = (request, model, project, agent_role, logger=None, provider=None, cancel=None, *, general_log=None, session_log=None, abort_on=Vec::new(), keyring_service=None, keyring_account=None))]
#[expect(clippy::too_many_arguments, reason = "the Python keyword arguments")]
fn send(
    py: Python<'_>,
    request: String,
    model: &str,
    project: &str,
    agent_role: &str,
    logger: Option<&Logger>,
    provider: Option<&str>,
    cancel: Option<&CancelToken>,
    general_log: Option<Py<PyAny>>,
    session_log: Option<Py<PyAny>>,
    abort_on: Vec<String>,
    keyring_service: Option<String>,
    keyring_account: Option<String>,
) -> String {
    let resolved = resolve_logger(logger, general_log, "decidr");
    let owned = match resolved {
        Ok(l) => l,
        Err(e) => return error_value(&e).to_string(),
    };
    decide_all(
        py,
        vec![request],
        model,
        project,
        agent_role,
        &owned,
        provider,
        1,
        cancel,
        session_log,
        abort_on,
        keyring_service,
        keyring_account,
    )
    .swap_remove(0)
    .to_string()
}

/// Decide every request on `workers` threads; one reply per request, in input order.
#[pyfunction]
#[pyo3(signature = (requests, model, project, agent_role, logger=None, provider=None, workers=8, cancel=None, *, general_log=None, session_log=None, abort_on=Vec::new(), keyring_service=None, keyring_account=None))]
#[expect(clippy::too_many_arguments, reason = "the Python keyword arguments")]
fn send_batch(
    py: Python<'_>,
    requests: Vec<String>,
    model: &str,
    project: &str,
    agent_role: &str,
    logger: Option<&Logger>,
    provider: Option<&str>,
    workers: usize,
    cancel: Option<&CancelToken>,
    general_log: Option<Py<PyAny>>,
    session_log: Option<Py<PyAny>>,
    abort_on: Vec<String>,
    keyring_service: Option<String>,
    keyring_account: Option<String>,
) -> Vec<String> {
    let owned = match resolve_logger(logger, general_log, "decidr") {
        Ok(l) => l,
        Err(e) => return vec![error_value(&e).to_string(); requests.len()],
    };
    decide_all(
        py,
        requests,
        model,
        project,
        agent_role,
        &owned,
        provider,
        workers,
        cancel,
        session_log,
        abort_on,
        keyring_service,
        keyring_account,
    )
    .iter()
    .map(Value::to_string)
    .collect()
}

/// sitr `run_query` with the injected Logger: one `sitr.query` record per call, then the
/// captures (or the `TreesitterError`) exactly as sc-treesitter returns them.
#[pyfunction]
#[pyo3(signature = (language, query, source, logger=None, captures=None, *, general_log=None))]
fn run_query(
    language: &str,
    query: &str,
    source: &Bound<'_, PyAny>,
    logger: Option<&Logger>,
    captures: Option<Vec<String>>,
    general_log: Option<Py<PyAny>>,
) -> PyResult<Vec<sc_treesitter::Capture>> {
    let logger = resolve_logger(logger, general_log, "sitr")
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
    let result = sc_treesitter::python::run_query(language, query, source, captures);
    let mut fields = Map::new();
    fields.insert("language".into(), language.into());
    fields.insert("ok".into(), result.is_ok().into());
    fields.insert(
        "captures".into(),
        result.as_ref().map_or(0, Vec::len).into(),
    );
    fields.insert(
        "error".into(),
        result.as_ref().err().map(ToString::to_string).into(),
    );
    record(logger.0.as_ref(), "sc_agent.sitr", "sitr.query", fields);
    logger.0.flush_records();
    result
}

/// `sc_agent.sitr`: the sitr Python API in this one wheel (ADR-SAG-0021); `run_query` takes
/// the same injected Logger as the decidr calls.
fn sitr(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    use sc_treesitter::python as ts;
    let m = PyModule::new(parent.py(), "sitr")?;
    m.add_function(wrap_pyfunction!(ts::languages, &m)?)?;
    m.add_function(wrap_pyfunction!(ts::queries, &m)?)?;
    m.add_function(wrap_pyfunction!(ts::query_source, &m)?)?;
    m.add_function(wrap_pyfunction!(run_query, &m)?)?;
    m.add_class::<sc_treesitter::Capture>()?;
    m.add(
        "TreesitterError",
        parent.py().get_type::<ts::TreesitterError>(),
    )?;
    parent.add_submodule(&m)?;
    let modules = parent.py().import("sys")?.getattr("modules")?;
    modules.set_item("sc_agent.sitr", &m)
}

#[pymodule]
#[pyo3(name = "_native")]
fn sc_agent(m: &Bound<'_, PyModule>) -> PyResult<()> {
    sitr(m)?;
    m.add_class::<Logger>()?;
    m.add_class::<CancelToken>()?;
    m.add_function(wrap_pyfunction!(logger, m)?)?;
    m.add_function(wrap_pyfunction!(log_root, m)?)?;
    m.add_function(wrap_pyfunction!(send, m)?)?;
    m.add_function(wrap_pyfunction!(send_batch, m)?)
}

fn resolve_logger(
    legacy: Option<&Logger>,
    injected: Option<Py<PyAny>>,
    service: &str,
) -> Result<Logger, Error> {
    if let Some(target) = injected {
        let root = sc_agent_log::log_root(service)?;
        return Ok(Logger(
            sc_agent_log::python::PythonSink::open(target)?,
            root,
        ));
    }
    if let Some(l) = legacy {
        return Ok(Logger(l.0.clone(), l.1.clone()));
    }
    let root = sc_agent_log::log_root(service)?;
    Ok(Logger(sc_agent_log::general(service, &root)?, root))
}

#[pyfunction]
fn log_root() -> (Option<String>, Option<String>) {
    match sc_agent_log::log_root("decidr") {
        Ok(root) => (Some(root.to_string_lossy().into_owned()), None),
        Err(e) => (None, Some(error_value(&e).to_string())),
    }
}
