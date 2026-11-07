//! Session log port (feature `obs`, ADR-SAG-0016 r-1 amendment).
//!
//! This crate links no `sc-observability*` crate. Composition roots adapt
//! their logger to [`v2::SessionLog`].

pub mod v2;
