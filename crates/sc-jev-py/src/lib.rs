//! Python bindings for the Jev check facade.
//!
//! `sc-jev-py` is a composition root (ADR-SAG-0011, ADR-SAG-0016). It calls
//! `sc-jev-check` in-process and adapts `sc-observability` 1.5.0
//! `v2::Logger` to [`sc_agent_types::observability::v2::SharedSessionLog`].
//! Payload bytes and Jev HTTP stay in the Rust crates. This crate does not
//! depend on `sc-jev`.
//!
//! # Session log
//!
//! Log root resolution, first match wins:
//!
//! 1. The `log_dir` argument.
//! 2. `SC_AGENT_LOG_DIR`.
//! 3. `SC_LOG_ROOT` (the `sc-observability` convention).
//! 4. `~/.sc-agent/log` (`%USERPROFILE%\.sc-agent\log` when `HOME` is unset).
//!
//! The active file is `{log_root}/logs/sc-jev-py.log.jsonl`. One JSONL line is
//! written per judge send and per session `ask`. The line carries
//! `session_id`, `backend`, `prompt_sha256`, and `operation`. It does not
//! carry API keys. Session files default to `{log_root}/sessions`, or
//! `SC_AGENT_SESSION_DIR` when that variable is set. `session_dir` overrides
//! both.
//!
//! # Examples
//!
//! ```text
//! import sc_jev_py as jev
//! result = jev.run_check("check.toml", backends=["jev"], dry_run=False)
//! ```

#![warn(missing_docs)]

mod error;
mod logging;
mod run;
mod scripted;

use pyo3::prelude::*;
use pyo3::types::PyModuleMethods;

use crate::error::JevError;

#[doc(inline)]
pub use run::{
    CheckResult, Session, build_request, canonical_bytes, canonical_sha256, default_log_root,
    log_file, resolve_log_root, resolve_session_dir, run_check,
};

/// Registers the Python entry points.
///
/// Log root resolution, first match wins: the `log_dir` argument,
/// `SC_AGENT_LOG_DIR`, `SC_LOG_ROOT`, then `~/.sc-agent/log`
/// (`%USERPROFILE%\.sc-agent\log` when `HOME` is unset). The JSONL file is
/// `{log_root}/logs/sc-jev-py.log.jsonl`. One line is appended per judge send
/// and per session `ask`. Lines contain `session_id`, `backend`,
/// `prompt_sha256`, and `operation`, and do not contain API keys.
///
/// Session files default to `{log_root}/sessions`. `SC_AGENT_SESSION_DIR`
/// overrides that default. The `session_dir` argument overrides both.
///
/// `build_request` and `run_check` call `sc-jev-check`. They do not
/// reimplement payload canonicalization or Jev HTTP. Pass `scripted_answers`
/// only for an in-process judge that never opens a socket. Omit it to resolve
/// keys and use the HTTP backends inside `sc-jev-check`.
#[pymodule]
fn sc_jev_py(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<CheckResult>()?;
    module.add_class::<Session>()?;
    module.add_function(wrap_pyfunction!(run::build_request, module)?)?;
    module.add_function(wrap_pyfunction!(run::run_check, module)?)?;
    module.add_function(wrap_pyfunction!(run::canonical_bytes, module)?)?;
    module.add_function(wrap_pyfunction!(run::canonical_sha256, module)?)?;
    module.add_function(wrap_pyfunction!(run::default_log_root, module)?)?;
    module.add_function(wrap_pyfunction!(run::resolve_log_root, module)?)?;
    module.add_function(wrap_pyfunction!(run::resolve_session_dir, module)?)?;
    module.add_function(wrap_pyfunction!(run::log_file, module)?)?;
    module.add("JevError", module.py().get_type::<JevError>())?;
    Ok(())
}
