//! decidr's library target: the composition root shared by the `decidr` binary and
//! sc-agent-py (ADR-SAG-0004).
//!
//! It chooses the backend (a configured local runner, TypeSafe, or an LLM provider by
//! ranking), wires one `DecisionRunner` over one `HttpClient`, reads requests within the
//! REQ-SAG-0014 limits, and lists models and diagnostics. Every operation returns
//! [`DecidrError`]; a failed decision is data in its `DecisionResult`.
//!
//! # Examples
//!
//! ```
//! use decidr::input::build_request;
//!
//! let json = br#"{"model":"jev-latest","state":"s","questions":{"q":{"type":"noul"}}}"#;
//! let request = build_request(json, &[], false, None)?;
//! assert_eq!(request.digests().len(), 1);
//! # Ok::<(), decidr::DecidrError>(())
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod input;
pub mod integration_gate;
pub mod output;
pub mod signal;

mod doctor;
mod error;
mod select;

#[doc(inline)]
pub use doctor::{DetectedKey, DoctorOptions, DoctorReport, MissingModel, Tie, doctor};
#[doc(inline)]
pub use error::DecidrError;
#[doc(inline)]
pub use select::{
    Context, ModelSelector, ModelsOptions, SendOptions, general_logger, load_config, models, send,
    send_batch, send_batch_on, send_on, session_identity, session_logger,
};
