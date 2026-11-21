//! The Python module. Every call returns `{"ok": true, "value": ...}` or
//! `{"ok": false, "error": {"id", "message"}}` as JSON text; nothing raises.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use pyo3::prelude::*;
use sc_agent_http::{Cancel, Policy};
use sc_agent_log::{Session, repo_id};
use sc_agent_types::Error;
use sc_decision_schema::DecisionRequest;
use serde_json::{Value, json};

fn reply(result: Result<Value, Error>) -> String {
    match result {
        Ok(value) => json!({ "ok": true, "value": value }),
        Err(e) => json!({ "ok": false, "error": { "id": e.id(), "message": e.to_string() } }),
    }
    .to_string()
}

/// Runs `f` with the GIL released; a panic becomes an error value.
fn guarded(py: Python<'_>, f: impl FnOnce() -> Result<Value, Error> + Send) -> String {
    let result = py.detach(|| catch_unwind(AssertUnwindSafe(f)));
    reply(result.unwrap_or_else(|_| Err(Error::Invalid("internal error (panic)".into()))))
}

/// Decide one request (JSON text) with `model` on `provider` (or the first with a key).
#[pyfunction]
#[pyo3(signature = (request, model, project, agent_role, provider=None, log_root=None))]
fn decide(py: Python<'_>, request: &str, model: &str, project: &str, agent_role: &str, provider: Option<&str>, log_root: Option<PathBuf>) -> String {
    guarded(py, || {
        let request: DecisionRequest = serde_json::from_str(request).map_err(|e| Error::Config(format!("request: {e}")))?;
        let cwd = std::env::current_dir().unwrap_or_default();
        let session = match log_root {
            Some(root) => Some(Session::open(&root, repo_id(&cwd), project, agent_role)?),
            None => None,
        };
        let (decision, id) = decidr::open(model, provider, Some(&cwd), Policy::default(), session)?;
        let outcome = decision.decide(DecisionRequest { model: id, ..request }, &Cancel::default());
        let value = outcome.result?;
        serde_json::to_value(value).map_err(|e| Error::ResponseParse(e.to_string()))
    })
}

#[pymodule]
fn sc_agent(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(decide, m)?)
}
