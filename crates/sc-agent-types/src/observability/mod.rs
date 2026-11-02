//! Temporary v2 logging port.
//!
//! Delete this module and the `sc-observability-types` dependency when upstream
//! v2 ships (bead `agt-obs-v2-shim-removal`). Full `sc-observability` belongs
//! on the composition roots `sc-ai` and `sc-jev-py`, not on this crate.

pub mod v2;
