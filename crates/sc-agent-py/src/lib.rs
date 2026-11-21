//! The Python module. Every call returns `{"ok": true, "value": ...}` or
//! `{"ok": false, "error": {"id", "message"}}` as JSON text; nothing raises.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::Arc;

use pyo3::prelude::*;
use sc_agent_http::{Cancel, Policy};
use sc_agent_log::{Session, record, repo_id};
use sc_agent_types::Error;
use sc_decision_schema::DecisionRequest;
use serde_json::{Map, Value, json};

/// `SC_DECIDR_LOG_ROOT` (tests), else `~/.sc-agent`.
fn log_root() -> PathBuf {
    match std::env::var_os("SC_DECIDR_LOG_ROOT").filter(|v| !v.is_empty()) {
        Some(root) => PathBuf::from(root),
        None => std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default().join(".sc-agent"),
    }
}

fn error_value(e: &Error) -> Value {
    json!({ "ok": false, "error": { "id": e.id(), "message": e.to_string() } })
}

fn reply(result: Result<Value, Error>) -> Value {
    match result {
        Ok(value) => json!({ "ok": true, "value": value }),
        Err(e) => error_value(&e),
    }
}

/// Runs `f` with the GIL released; a panic becomes an error value.
fn guarded<T: Send>(py: Python<'_>, f: impl FnOnce() -> Result<T, Error> + Send) -> Result<T, Error> {
    py.detach(|| catch_unwind(AssertUnwindSafe(f))).unwrap_or_else(|_| Err(Error::Invalid("internal error (panic)".into())))
}

/// The general logger: obtained from this module, passed back into every call.
#[pyclass(frozen)]
struct Logger(Arc<sc_observability::v2::Logger>);

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

/// `(Logger, None)` writing to `~/.sc-agent/logs/<name>.log.jsonl`, or `(None, error JSON)`.
#[pyfunction]
fn logger(name: &str) -> (Option<Logger>, Option<String>) {
    match sc_agent_log::general(name, &log_root()) {
        Ok(l) => (Some(Logger(l)), None),
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
) -> Vec<Value> {
    let count = requests.len();
    let cancel = cancel.map(|c| c.0.clone()).unwrap_or_default();
    let result = guarded(py, || {
        if workers == 0 {
            return Err(Error::Config("workers must be at least 1".into()));
        }
        let requests = requests
            .iter()
            .enumerate()
            .map(|(i, r)| serde_json::from_str::<DecisionRequest>(r).map_err(|e| Error::Config(format!("request {i}: {e}"))))
            .collect::<Result<Vec<_>, _>>()?;
        let cwd = std::env::current_dir().unwrap_or_default();
        let session = Session::open(&log_root(), repo_id(&cwd), project, agent_role)?;
        let (decision, id) = decidr::open(model, provider, Some(&cwd), Policy::default(), Some(session))?;
        let requests = requests.into_iter().map(|r| DecisionRequest { model: id.clone(), ..r }).collect();
        let mut replies = vec![Value::Null; count];
        decision.decide_batch(requests, workers, &cancel, &mut |outcome| {
            let value = outcome.result.and_then(|r| serde_json::to_value(r).map_err(|e| Error::ResponseParse(e.to_string())));
            if let Some(slot) = replies.get_mut(outcome.position) {
                *slot = reply(value);
            }
        });
        Ok(replies)
    });
    let replies = result.unwrap_or_else(|e| vec![error_value(&e); count.max(1)]);
    for r in &replies {
        let mut fields = Map::new();
        fields.insert("model".into(), model.into());
        fields.insert("ok".into(), r.get("ok").cloned().unwrap_or_default());
        fields.insert("error_id".into(), r.pointer("/error/id").cloned().unwrap_or_default());
        record(&logger.0, "sc_agent.decidr", "decidr.send", fields);
    }
    let _ = logger.0.flush();
    replies
}

/// Decide one request (JSON text) with `model` on `provider` (or the first with a key).
#[pyfunction]
#[pyo3(signature = (request, model, project, agent_role, logger, provider=None, cancel=None))]
#[expect(clippy::too_many_arguments, reason = "the Python keyword arguments")]
fn send(py: Python<'_>, request: String, model: &str, project: &str, agent_role: &str, logger: &Logger, provider: Option<&str>, cancel: Option<&CancelToken>) -> String {
    decide_all(py, vec![request], model, project, agent_role, logger, provider, 1, cancel).swap_remove(0).to_string()
}

/// Decide every request on `workers` threads; one reply per request, in input order.
#[pyfunction]
#[pyo3(signature = (requests, model, project, agent_role, logger, provider=None, workers=8, cancel=None))]
#[expect(clippy::too_many_arguments, reason = "the Python keyword arguments")]
fn send_batch(
    py: Python<'_>,
    requests: Vec<String>,
    model: &str,
    project: &str,
    agent_role: &str,
    logger: &Logger,
    provider: Option<&str>,
    workers: usize,
    cancel: Option<&CancelToken>,
) -> Vec<String> {
    decide_all(py, requests, model, project, agent_role, logger, provider, workers, cancel).iter().map(Value::to_string).collect()
}

#[pymodule]
fn sc_agent(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Logger>()?;
    m.add_class::<CancelToken>()?;
    m.add_function(wrap_pyfunction!(logger, m)?)?;
    m.add_function(wrap_pyfunction!(send, m)?)?;
    m.add_function(wrap_pyfunction!(send_batch, m)?)
}
