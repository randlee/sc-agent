//! PyO3 support types (feature `python`).
//!
//! The functions here mirror the Rust API. [`register`] adds them, [`Capture`] and
//! [`TreesitterError`] to whichever `#[pymodule]` owns them; that module is built by the
//! `sc-treesitter-py` crate. Every [`Error`] becomes `TreesitterError(id, message)`.

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyModule, PyString};

use crate::{Capture, Error, LanguageId};

pyo3::create_exception!(
    sc_treesitter,
    TreesitterError,
    pyo3::exceptions::PyException,
    "An sc-treesitter error; `args` is `(id, message)`."
);

impl From<Error> for PyErr {
    fn from(error: Error) -> Self {
        TreesitterError::new_err((error.id(), error.message()))
    }
}

/// Returns the ids of the grammars compiled in.
#[pyfunction]
#[must_use]
pub fn languages() -> Vec<&'static str> {
    crate::languages().map(|id| id.as_str()).collect()
}

/// Returns the predefined queries as `(language, query)` pairs.
///
/// # Errors
///
/// `TreesitterError` when `language` is set and not compiled in.
#[pyfunction]
#[pyo3(signature = (language = None))]
pub fn queries(language: Option<&str>) -> PyResult<Vec<(&'static str, &'static str)>> {
    let language = language.map(LanguageId::new).transpose()?;
    Ok(crate::queries(language)
        .into_iter()
        .map(|info| (info.language, info.name))
        .collect())
}

/// Returns the text of a predefined query.
///
/// # Errors
///
/// `TreesitterError` for an unknown language or query.
#[pyfunction]
pub fn query_source(language: &str, name: &str) -> PyResult<&'static str> {
    Ok(crate::query_source(LanguageId::new(language)?, name)?)
}

/// Runs `query` against `source` (`bytes` or `str`) and returns its captures.
///
/// The GIL is released while tree-sitter parses and queries.
///
/// # Errors
///
/// `TreesitterError` with id `treesitter.python` when `source` is neither `bytes` nor `str`
/// (or a `str` that cannot be encoded as UTF-8), and with the [`crate::run_query`] ids
/// otherwise.
#[pyfunction]
#[pyo3(signature = (language, query, source, captures = None))]
pub fn run_query(
    language: &str,
    query: &str,
    source: &Bound<'_, PyAny>,
    captures: Option<Vec<String>>,
) -> PyResult<Vec<Capture>> {
    let language = LanguageId::new(language)?;
    let python_error = |error: PyErr| Error::Python {
        detail: error.to_string(),
    };
    let bytes: &[u8] = if let Ok(bytes) = source.cast::<PyBytes>() {
        bytes.as_bytes()
    } else {
        let text = source
            .cast::<PyString>()
            .map_err(|error| python_error(error.into()))?;
        text.to_str().map_err(python_error)?.as_bytes()
    };
    let names: Option<Vec<&str>> = captures
        .as_ref()
        .map(|names| names.iter().map(String::as_str).collect());
    let result = source
        .py()
        .detach(|| crate::run_query(language, query, bytes, None, names.as_deref()));
    Ok(result?)
}

/// Adds the functions, `Capture` and `TreesitterError` to `module`.
///
/// # Errors
///
/// Any `PyErr` raised while adding the items.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(languages, module)?)?;
    module.add_function(wrap_pyfunction!(queries, module)?)?;
    module.add_function(wrap_pyfunction!(query_source, module)?)?;
    module.add_function(wrap_pyfunction!(run_query, module)?)?;
    module.add_class::<Capture>()?;
    module.add("TreesitterError", module.py().get_type::<TreesitterError>())?;
    Ok(())
}
