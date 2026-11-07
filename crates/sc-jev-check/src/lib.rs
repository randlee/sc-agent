//! Jev check engine: `check.toml`, extraction, the canonical payload,
//! backends, sessions, and the one run API.
//!
//! Run a check with [`run::prepare`] then [`run::run`]; the result is one
//! [`run::RunResponse`] whose serialization every consumer prints
//! (ADR-SAG-0024). Module roles and boundaries are described in
//! `docs/sc-jev-check/architecture.md`.

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
