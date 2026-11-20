//! Python entry points for tree-sitter with one general-log record per call.

use ::sc_treesitter::{Capture, python::TreesitterError};
use pyo3::prelude::*;
use pyo3::types::PyModule;
use sc_agent_log::{GeneralLogger, Level, LogContext, LogError};
use std::error::Error;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

/// The default sitr logger, opened once per log root and kept for the process.
static DEFAULT: Mutex<Option<(Option<PathBuf>, GeneralLogger)>> = Mutex::new(None);

/// The caller's logger, or the default sitr log under sitr's log root.
fn logger(target: Option<Py<PyAny>>) -> Option<GeneralLogger> {
    let result = match target {
        Some(target) => GeneralLogger::injected_python(target, "sitr"),
        None => default_logger(),
    };
    result.inspect_err(notice).ok()
}

fn default_logger() -> Result<GeneralLogger, LogError> {
    let root = std::env::var_os("SITR_LOG_ROOT");
    let root = sc_agent_log::cli_log_root(root.as_deref(), sc_agent_log::resolve_home());
    let mut cached = DEFAULT.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((opened, log)) = cached.as_ref()
        && *opened == root
    {
        return Ok(log.clone());
    }
    let log = GeneralLogger::open_default(root.as_deref(), "sitr")?;
    *cached = Some((root, log.clone()));
    Ok(log)
}

fn notice(error: &LogError) {
    eprintln!("sitr logging failed: {error}");
    let mut cause = error.source();
    while let Some(error) = cause {
        eprintln!("sitr logging cause: {error}");
        cause = error.source();
    }
}

/// Record the result without including the query or source contents.
fn record<T>(
    log: Option<GeneralLogger>,
    action: &'static str,
    language: Option<&str>,
    query_size: usize,
    result: &PyResult<T>,
    py: Python<'_>,
) {
    let Some(log) = log else { return };
    let error_id = result
        .as_ref()
        .err()
        .filter(|error| error.is_instance_of::<TreesitterError>(py))
        .and_then(|error| {
            error
                .value(py)
                .getattr("args")
                .ok()?
                .get_item(0)
                .ok()?
                .extract::<String>()
                .ok()
        });
    let fields = [
        ("language".to_owned(), language.map(str::to_owned).into()),
        ("query_size".to_owned(), query_size.into()),
        (
            "outcome".to_owned(),
            if result.is_ok() { "ok" } else { "error" }.into(),
        ),
        ("error_id".to_owned(), error_id.into()),
    ]
    .into_iter()
    .collect();
    let level = if result.is_ok() {
        Level::Info
    } else {
        Level::Error
    };
    if let Err(error) = log.write(level, "sitr", action, LogContext::NONE, fields) {
        notice(&error);
    }
    // The default logger stays open in `DEFAULT`, so this flushes; it never shuts it down.
    if let Err(error) = log.close() {
        notice(&error);
    }
}

#[pyfunction]
#[pyo3(signature = (*, general_log = None))]
fn languages(py: Python<'_>, general_log: Option<Py<PyAny>>) -> PyResult<Vec<&'static str>> {
    let log = logger(general_log);
    let result = Ok(::sc_treesitter::python::languages());
    record(log, "sitr.languages", None, 0, &result, py);
    result
}

#[pyfunction]
#[pyo3(signature = (language = None, *, general_log = None))]
fn queries(
    py: Python<'_>,
    language: Option<&str>,
    general_log: Option<Py<PyAny>>,
) -> PyResult<Vec<(&'static str, &'static str)>> {
    let log = logger(general_log);
    let result = ::sc_treesitter::python::queries(language);
    record(log, "sitr.queries", language, 0, &result, py);
    result
}

#[pyfunction]
#[pyo3(signature = (language, name, *, general_log = None))]
fn query_source(
    py: Python<'_>,
    language: &str,
    name: &str,
    general_log: Option<Py<PyAny>>,
) -> PyResult<&'static str> {
    let log = logger(general_log);
    let result = ::sc_treesitter::python::query_source(language, name);
    record(
        log,
        "sitr.query_source",
        Some(language),
        result.as_ref().map_or(0, |query| query.len()),
        &result,
        py,
    );
    result
}

#[pyfunction]
#[pyo3(signature = (language, query, source, captures = None, *, general_log = None))]
fn run_query(
    py: Python<'_>,
    language: &str,
    query: &str,
    source: &Bound<'_, PyAny>,
    captures: Option<Vec<String>>,
    general_log: Option<Py<PyAny>>,
) -> PyResult<Vec<Capture>> {
    let log = logger(general_log);
    let result = ::sc_treesitter::python::run_query(language, query, source, captures);
    record(
        log,
        "sitr.run_query",
        Some(language),
        query.len(),
        &result,
        py,
    );
    result
}

#[pyo3::pymodule]
fn sc_treesitter(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(languages, module)?)?;
    module.add_function(wrap_pyfunction!(queries, module)?)?;
    module.add_function(wrap_pyfunction!(query_source, module)?)?;
    module.add_function(wrap_pyfunction!(run_query, module)?)?;
    module.add_class::<Capture>()?;
    module.add("TreesitterError", module.py().get_type::<TreesitterError>())?;
    Ok(())
}
