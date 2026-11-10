//! Shared value types for sc-agent: the bottom layer of the workspace (ADR-SAG-0004).
//!
//! This crate holds the types every other sc-agent crate speaks in:
//!
//! - the model catalog ([`Catalog`], [`ModelId`], [`ShortName`], [`ReleaseDate`], [`MODELS`])
//!   and how a caller picks a model ([`ModelChoice`]);
//! - the providers that serve models ([`Provider`]), separate from model ids (REQ-SAG-0103);
//! - session ids ([`SessionId`], a ULID) and environment-variable names ([`VarName`]);
//! - cooperative cancellation with deadline waits and wake hooks ([`CancelToken`]);
//! - the one error convention of the workspace ([`ErrorId`], [`ErrorDetail`]).
//!
//! It does no I/O, reads no files and depends on no runtime: only `serde` and `ulid`.
#![forbid(unsafe_code)]

mod cancel;
mod catalog;
mod error;
mod error_id;
mod model;
mod provider;
mod release_date;
mod session_id;
mod short_name;
mod var_name;

#[doc(inline)]
pub use cancel::{CancelHook, CancelToken, Wait};
#[doc(inline)]
pub use catalog::{Catalog, ModelChoice};
#[doc(inline)]
pub use error::TypesError;
#[doc(inline)]
pub use error_id::{ErrorDetail, ErrorId};
#[doc(inline)]
pub use model::{MODELS, ModelDescriptor, ModelId};
#[doc(inline)]
pub use provider::Provider;
#[doc(inline)]
pub use release_date::ReleaseDate;
#[doc(inline)]
pub use session_id::SessionId;
#[doc(inline)]
pub use short_name::ShortName;
#[doc(inline)]
pub use var_name::VarName;
