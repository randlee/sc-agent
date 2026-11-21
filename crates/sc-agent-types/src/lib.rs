#![forbid(unsafe_code)]
//! Shared types: the error value and the provider/model table.

mod error;
pub mod providers;

pub use error::{BusyReason, Error};
