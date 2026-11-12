//! The `sc_treesitter` Python extension module: sc-treesitter's PyO3 bindings.
//!
//! The functions and types live in `sc_treesitter::python` (feature `python`); this crate
//! only owns the `#[pymodule]`.

#[pyo3::pymodule]
fn sc_treesitter(module: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> {
    // `#[pymodule]` defines a module named `sc_treesitter` here; `::` names the library crate.
    ::sc_treesitter::python::register(module)
}
