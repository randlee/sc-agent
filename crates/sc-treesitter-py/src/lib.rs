//! Python entry points for tree-sitter with one general-log record per call.

use std::path::PathBuf;

use ::sc_treesitter::Capture;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use sc_agent_log::{GeneralLogger, Level, LogContext};

/// Open the caller's logger or the default sitr log.
fn logger(target: Option<Py<PyAny>>, root: Option<&std::path::Path>) -> PyResult<GeneralLogger> {
    let result = match target {
        Some(target) => GeneralLogger::injected_python(target, "sitr"),
        None => GeneralLogger::open_default(root, "sitr"),
    };
    result.map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(error.to_string()))
}

/// Record the result without including the query or source contents.
fn record<T>(
    log: GeneralLogger,
    action: &'static str,
    language: Option<&str>,
    query_size: usize,
    result: &PyResult<T>,
    py: Python<'_>,
) {
    let error_id = result.as_ref().err().and_then(|error| {
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
        eprintln!("sitr logging failed: {error}");
    }
    if let Err(error) = log.close() {
        eprintln!("sitr logging failed: {error}");
    }
}

#[pyfunction]
#[pyo3(signature = (*, general_log = None, log_root = None))]
fn languages(
    py: Python<'_>,
    general_log: Option<Py<PyAny>>,
    log_root: Option<PathBuf>,
) -> PyResult<Vec<&'static str>> {
    let log = logger(general_log, log_root.as_deref())?;
    let result = Ok(::sc_treesitter::python::languages());
    record(log, "sitr.languages", None, 0, &result, py);
    result
}

#[pyfunction]
#[pyo3(signature = (language = None, *, general_log = None, log_root = None))]
fn queries(
    py: Python<'_>,
    language: Option<&str>,
    general_log: Option<Py<PyAny>>,
    log_root: Option<PathBuf>,
) -> PyResult<Vec<(&'static str, &'static str)>> {
    let log = logger(general_log, log_root.as_deref())?;
    let result = ::sc_treesitter::python::queries(language);
    record(log, "sitr.queries", language, 0, &result, py);
    result
}

#[pyfunction]
#[pyo3(signature = (language, name, *, general_log = None, log_root = None))]
fn query_source(
    py: Python<'_>,
    language: &str,
    name: &str,
    general_log: Option<Py<PyAny>>,
    log_root: Option<PathBuf>,
) -> PyResult<&'static str> {
    let log = logger(general_log, log_root.as_deref())?;
    let result = ::sc_treesitter::python::query_source(language, name);
    record(
        log,
        "sitr.query_source",
        Some(language),
        name.len(),
        &result,
        py,
    );
    result
}

#[pyfunction]
#[pyo3(signature = (language, query, source, captures = None, *, general_log = None, log_root = None))]
fn run_query(
    py: Python<'_>,
    language: &str,
    query: &str,
    source: &Bound<'_, PyAny>,
    captures: Option<Vec<String>>,
    general_log: Option<Py<PyAny>>,
    log_root: Option<PathBuf>,
) -> PyResult<Vec<Capture>> {
    let log = logger(general_log, log_root.as_deref())?;
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
    module.add(
        "TreesitterError",
        module
            .py()
            .get_type::<::sc_treesitter::python::TreesitterError>(),
    )?;
    Ok(())
}
