//! TypeSafe Jev HTTP surface.
//!
//! Phase A implements the check/judge call. Later Jev endpoints extend this
//! module in place (ADR-SAG-0012). No other crate sends Jev API traffic.

mod config;

#[doc(inline)]
pub use config::JevClientConfig;
