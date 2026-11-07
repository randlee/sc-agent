//! Check facade module tree.
//!
//! Module roles and boundaries are described in `docs/architecture.md`.

pub mod config;
pub mod equal_input;
pub mod extract;
pub mod payload;
pub mod registry;
pub mod run;
pub mod session;

/// Shared bounded subprocess runner for unit tests (FTQ-008).
#[cfg(test)]
#[path = "../../../tests/support/deadline.rs"]
mod deadline;
