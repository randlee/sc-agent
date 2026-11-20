use pyo3::{prelude::*, types::PyDict};
use serde_json::Value;

fn as_python<'py>(py: Python<'py>, value: &Value) -> Bound<'py, PyAny> {
    py.import("json")
        .and_then(|m| m.call_method1("loads", (value.to_string(),)))
        .unwrap_or_else(|_| py.None().into_bound(py))
}

fn event(py: Python<'_>, handle: Option<&Bound<'_, PyAny>>, action: &str) -> Result<(), Value> {
    if let Some(handle) = handle {
        let kind = handle.get_type();
        let module = kind
            .getattr("__module__")
            .and_then(|v| v.extract::<String>())
            .unwrap_or_default();
        let name = kind.name().map(|v| v.to_string()).unwrap_or_default();
        if module != "sc_agent" || name != "Handle" {
            return Err(sc_agent::error(
                "invalid_logger",
                "Logger must be a sc-decidr handle",
                "Open a logger by client name",
            ));
        }
        let record = PyDict::new(py);
        let _ = record.set_item("action", action);
        if handle.call1((record,)).is_err() {
            return Err(sc_agent::error(
                "log_write",
                "Cannot write log",
                "Check logger",
            ));
        }
    } else if let Ok(log) = sc_agent::open_default_logger("sitr", "SITR_LOG_ROOT") {
        sc_agent::log(&log, action)?;
    } else {
        return Err(sc_agent::error(
            "log_open",
            "Cannot open log",
            "Check log directory",
        ));
    }
    Ok(())
}

#[pyfunction(signature = (general_log=None))]
fn languages<'py>(py: Python<'py>, general_log: Option<&Bound<'py, PyAny>>) -> Bound<'py, PyAny> {
    if let Err(e) = event(py, general_log, "sitr.languages") {
        return as_python(py, &e);
    }
    as_python(py, &::sc_treesitter::languages())
}

#[pyfunction(signature = (language, query, source, general_log=None))]
fn run_query<'py>(
    py: Python<'py>,
    language: &Bound<'py, PyAny>,
    query: &Bound<'py, PyAny>,
    source: &Bound<'py, PyAny>,
    general_log: Option<&Bound<'py, PyAny>>,
) -> Bound<'py, PyAny> {
    let (Ok(language), Ok(query), Ok(source)) = (
        language.extract::<String>(),
        query.extract::<String>(),
        source.extract::<String>(),
    ) else {
        return as_python(
            py,
            &sc_agent::error(
                "invalid_query",
                "Invalid query arguments",
                "Provide language, query, and source as strings",
            ),
        );
    };
    if let Err(e) = event(py, general_log, "sitr.query") {
        return as_python(py, &e);
    }
    as_python(py, &::sc_treesitter::run_query(&language, &query, &source))
}

#[pymodule]
mod sc_treesitter {
    #[pymodule_export]
    use super::{languages, run_query};
}
