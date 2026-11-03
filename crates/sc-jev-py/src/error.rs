//! Python `JevError` with the stable code and fix hint.

use pyo3::exceptions::PyException;
use pyo3::prelude::*;

pyo3::create_exception!(
    sc_jev_py,
    JevError,
    PyException,
    "Jev check failed. `code` is the stable id and `message` is the fix hint."
);

/// Builds a Python `JevError` from `error`.
///
/// `code` and `message` are attributes. The display text is `CODE: hint`.
/// Neither attribute is a place for secrets; `message` is [`sc_agent_types::JevError::fix_hint`].
pub(crate) fn to_py_err(py: Python<'_>, error: &sc_agent_types::JevError) -> PyErr {
    let code = error.code();
    let message = error.fix_hint();
    let err = PyErr::new::<JevError, _>(format!("{code}: {message}"));
    let value = err.value(py);
    let _ = value.setattr("code", code);
    let _ = value.setattr("message", message);
    err
}

pub(crate) fn map_err<T>(
    py: Python<'_>,
    result: Result<T, sc_agent_types::JevError>,
) -> PyResult<T> {
    result.map_err(|error| to_py_err(py, &error))
}
