//! The Python module. Every call returns `{"ok": true, "value": ...}` or
//! `{"ok": false, "error": {"id", "message"}}` as JSON text; nothing raises.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::Arc;

use pyo3::prelude::*;
use sc_agent_http::Cancel;
use sc_agent_log::{Session, record, repo_id};
use sc_agent_types::Error;
use sc_decision_schema::DecisionRequest;
use serde_json::{Map, Value, json};

fn error_value(e: &Error, attempts: u32) -> Value {
    let mut value = json!({ "ok": false, "error": { "id": e.id(), "message": e.to_string(), "attempts": attempts, "category": match e {
        Error::ProviderBusy { .. } => "busy", Error::Network(_) => "network",
        Error::QuotaExhausted { .. } => "quota", Error::MissingKey { .. } | Error::Keyring(_) => "auth",
        Error::ApiStatus { status: 401 | 403, .. } => "auth",
        Error::Config(_) => "config",
        Error::InvalidRequestTimeout(_) | Error::InvalidWorkers(_) => "validation", Error::Cancelled { .. } => "cancelled",
        Error::Log(_) => "log", _ => "other",
    } } });
    if let Some(error) = value.get_mut("error").and_then(Value::as_object_mut) {
        match e {
            Error::ApiStatus { status, .. } | Error::QuotaExhausted { status, .. } => {
                error.insert("status".into(), (*status).into());
            }
            Error::ProviderBusy {
                retry_after_seconds: Some(seconds),
                ..
            } => {
                error.insert("retry_after_ms".into(), seconds.saturating_mul(1000).into());
            }
            _ => {}
        }
    }
    value
}

fn reply(result: Result<Value, Error>, attempts: u32) -> Value {
    match result {
        Ok(value) => json!({ "ok": true, "value": value }),
        Err(e) => error_value(&e, attempts),
    }
}

/// The stored category of a reply's error value (a dict or JSON text).
#[pyfunction]
fn category(error: &Bound<'_, PyAny>) -> String {
    if let Ok(text) = error.extract::<String>() {
        return serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| v.get("category").and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_else(|| "other".into());
    }
    error
        .get_item("category")
        .ok()
        .and_then(|v| v.extract().ok())
        .unwrap_or_else(|| "other".into())
}

/// True only for busy and network errors, after the wheel's own resends.
#[pyfunction]
fn retryable(error: &Bound<'_, PyAny>) -> bool {
    matches!(category(error).as_str(), "busy" | "network")
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
struct Logger(Arc<sc_observability::v2::Logger>, PathBuf);

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
/// `sc_agent_log::log_root(name)` (`<NAME>_LOG_ROOT`, else `~/.sc-agent`), or `(None, error JSON)`. For
/// `name == "decidr"`, `DECIDR_LOG_ROOT` is a supported override; the shared rule applies to other names.
#[pyfunction]
fn logger(name: &str) -> (Option<Logger>, Option<String>) {
    let opened = sc_agent_log::log_root(name)
        .and_then(|root| Ok(Logger(sc_agent_log::general(name, &root)?, root)));
    match opened {
        Ok(l) => (Some(l), None),
        Err(e) => (None, Some(error_value(&e, 0).to_string())),
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
    workers: Option<&Bound<'_, PyAny>>,
    timeout_secs: Option<&Bound<'_, PyAny>>,
    keyring_service: Option<&Bound<'_, PyAny>>,
    keyring_account: Option<&Bound<'_, PyAny>>,
    cancel: Option<&CancelToken>,
    on_result: Option<Py<PyAny>>,
) -> Vec<Value> {
    let count = requests.len();
    let cancel = cancel.map(|c| c.0.clone()).unwrap_or_default();
    let notify = |value: Value, position: usize, session_id: String| {
        if let Some(cb) = &on_result {
            let mut line = value
                .as_object()
                .cloned()
                .ok_or_else(|| Error::Config("callback reply must be an object".into()))?;
            line.insert("position".into(), position.into());
            line.insert("session_id".into(), session_id.into());
            Python::attach(|py| cb.call1(py, (Value::Object(line).to_string(),)))
                .map_err(|e| Error::Config(format!("on_result raised: {e}")))?;
        }
        Ok::<_, Error>(value)
    };
    let options = (|| {
        let numeric = |v: &Bound<'_, PyAny>| {
            if v.is_instance_of::<pyo3::types::PyBool>() {
                f64::NAN
            } else {
                v.extract::<f64>().unwrap_or(f64::NAN)
            }
        };
        let integer = |v: &Bound<'_, PyAny>| {
            if v.is_instance_of::<pyo3::types::PyBool>() {
                i64::MIN
            } else {
                v.extract::<i64>().unwrap_or(i64::MIN)
            }
        };
        let (timeout, workers) =
            decidr::validate_limits(timeout_secs.map(numeric), workers.map(integer))?;
        let text = |v: &Bound<'_, PyAny>| {
            v.extract::<String>()
                .map_err(|_| Error::Config("keyring service/account must be strings".into()))
        };
        let service = keyring_service.map(text).transpose()?;
        let account = keyring_account.map(text).transpose()?;
        Ok::<_, Error>((timeout, workers, service, account))
    })();
    let result = options.and_then(|(timeout, workers, service, account)| {
        guarded(py, || {
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
            let root = sc_agent_config::repo_root();
            let session = Session::open(&logger.1, repo_id(&cwd), project, agent_role)?;
            let config = sc_agent_config::config(Some(project), root.as_deref())?.config;
            let mut replies = vec![Value::Null; count];
            decidr::send(
                requests,
                model,
                provider,
                None,
                &config,
                workers,
                timeout,
                root.as_deref(),
                Some(session),
                Some((service.as_deref(), account.as_deref())),
                &logger.0,
                &cancel,
                &mut |outcome| {
                    let value = outcome.result.as_ref().map_err(Clone::clone).and_then(|r| {
                        serde_json::to_value(r).map_err(|e| Error::ResponseParse(e.to_string()))
                    });
                    if let Some(slot) = replies.get_mut(outcome.position) {
                        let notified = notify(
                            reply(value, outcome.sends),
                            outcome.position,
                            outcome.session.to_string(),
                        );
                        if let Err(e) = &notified {
                            outcome.result = Err(e.clone());
                        }
                        *slot = notified.unwrap_or_else(|e| error_value(&e, outcome.sends));
                    }
                },
            )?;
            Ok(replies)
        })
    });
    let failed = result.is_err();
    let replies = result.unwrap_or_else(|e| {
        (0..count)
            .map(|position| {
                notify(error_value(&e, 0), position, ulid::Ulid::new().to_string())
                    .unwrap_or_else(|e| error_value(&e, 0))
            })
            .collect()
    });
    if failed {
        for r in &replies {
            let mut fields = Map::new();
            fields.insert("model".into(), model.into());
            fields.insert("ok".into(), r.get("ok").cloned().unwrap_or_default());
            fields.insert(
                "error_id".into(),
                r.pointer("/error/id").cloned().unwrap_or_default(),
            );
            record(&logger.0, "sc_agent.decidr", "decidr.send", fields);
        }
    }
    let _ = logger.0.flush();
    replies
}

/// Decide one request (JSON text) with `model` on `provider` (or the first with a key).
#[pyfunction]
#[pyo3(signature = (request, model, project, agent_role, logger, provider=None, cancel=None, *, timeout_secs=None, keyring_service=None, keyring_account=None))]
#[expect(clippy::too_many_arguments, reason = "the Python keyword arguments")]
fn send(
    py: Python<'_>,
    request: String,
    model: &str,
    project: &str,
    agent_role: &str,
    logger: &Logger,
    provider: Option<&str>,
    cancel: Option<&CancelToken>,
    timeout_secs: Option<&Bound<'_, PyAny>>,
    keyring_service: Option<&Bound<'_, PyAny>>,
    keyring_account: Option<&Bound<'_, PyAny>>,
) -> String {
    decide_all(
        py,
        vec![request],
        model,
        project,
        agent_role,
        logger,
        provider,
        None,
        timeout_secs,
        keyring_service,
        keyring_account,
        cancel,
        None,
    )
    .swap_remove(0)
    .to_string()
}

/// Decide every request on `workers` threads; one reply per request, in input order.
#[pyfunction]
#[pyo3(signature = (requests, model, project, agent_role, logger, provider=None, workers=None, cancel=None, *, timeout_secs=None, keyring_service=None, keyring_account=None, on_result=None))]
#[expect(clippy::too_many_arguments, reason = "the Python keyword arguments")]
fn send_batch(
    py: Python<'_>,
    requests: Vec<String>,
    model: &str,
    project: &str,
    agent_role: &str,
    logger: &Logger,
    provider: Option<&str>,
    workers: Option<&Bound<'_, PyAny>>,
    cancel: Option<&CancelToken>,
    timeout_secs: Option<&Bound<'_, PyAny>>,
    keyring_service: Option<&Bound<'_, PyAny>>,
    keyring_account: Option<&Bound<'_, PyAny>>,
    on_result: Option<Py<PyAny>>,
) -> Vec<String> {
    decide_all(
        py,
        requests,
        model,
        project,
        agent_role,
        logger,
        provider,
        workers,
        timeout_secs,
        keyring_service,
        keyring_account,
        cancel,
        on_result,
    )
    .iter()
    .map(Value::to_string)
    .collect()
}

/// sitr `run_query` with the injected Logger: one `sitr.query` record per call, then the
/// captures (or the `TreesitterError`) exactly as sc-treesitter returns them.
#[pyfunction]
#[pyo3(signature = (language, query, source, logger, captures=None))]
fn run_query(
    language: &str,
    query: &str,
    source: &Bound<'_, PyAny>,
    logger: &Logger,
    captures: Option<Vec<String>>,
) -> PyResult<Vec<sc_treesitter::Capture>> {
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
    record(&logger.0, "sc_agent.sitr", "sitr.query", fields);
    let _ = logger.0.flush();
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
fn sc_agent(m: &Bound<'_, PyModule>) -> PyResult<()> {
    sitr(m)?;
    m.add_class::<Logger>()?;
    m.add_class::<CancelToken>()?;
    m.add_function(wrap_pyfunction!(logger, m)?)?;
    m.add_function(wrap_pyfunction!(send, m)?)?;
    m.add_function(wrap_pyfunction!(category, m)?)?;
    m.add_function(wrap_pyfunction!(retryable, m)?)?;
    m.add_function(wrap_pyfunction!(send_batch, m)?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn api_auth_status_is_preserved_and_classified() {
        for status in [401, 403] {
            let value = super::error_value(
                &sc_agent_types::Error::ApiStatus {
                    status,
                    body: "refused".into(),
                },
                1,
            );
            assert_eq!(
                value.pointer("/error/category"),
                Some(&serde_json::json!("auth"))
            );
            assert_eq!(
                value.pointer("/error/status"),
                Some(&serde_json::json!(status))
            );
            assert_eq!(
                value.pointer("/error/attempts"),
                Some(&serde_json::json!(1))
            );
        }
    }
}
