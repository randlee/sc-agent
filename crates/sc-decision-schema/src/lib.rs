//! The decision-layer contract of sc-agent: TypeSafe System One API types and the decision traits.
//!
//! - The one request form: [`SystemOneRequest`] (POST /v1/systemone) with its
//!   [`Question`]s and [`Asset`]s, wrapped with its image parts in [`DecisionRequest`], valid
//!   by construction.
//! - The one response form: [`SystemOneResponse`] with its [`Answer`]s (ADR-SAG-0001), plus
//!   [`ModelMetadataList`] (GET /v1/models) and the documented 400/422 bodies.
//! - [`Backend`]: the sans-IO translation every decision backend implements.
//! - [`Decision`]: the sync decide / decide-stream API the runner implements, returning
//!   [`DecisionResult`]s.
//! - [`QuestionDigest`]: the per-question sc-sha digest of everything sent (REQ-SAG-0202).
//! - [`DecisionError`]: the one error type, with stable `decision.*` ids (REQ-SAG-0002).
//!
//! Every type round-trips byte-identical through serde_json: key order is kept
//! ([`OrderedMap`], serde_json `preserve_order`) and floats print exactly (`float_roundtrip`,
//! REQ-SAG-0007). The crate does no I/O, holds no HTTP client, retry, cache or log code, and
//! starts no runtime (ANC-001, REQ-SAG-0009). It sits above sc-agent-types and sc-agent-http
//! and below every decision backend and the runner (ADR-SAG-0004).
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod api_error;
mod backend;
mod decision;
mod digest;
mod error;
mod map;
mod models;
mod request;
mod response;

#[doc(inline)]
pub use api_error::{HttpBadRequestError, HttpValidationError, LocItem, ValidationError};
#[doc(inline)]
pub use backend::Backend;
#[doc(inline)]
pub use decision::{
    Decision, DecisionRequest, DecisionResult, ImagePart, MAX_IMAGE_BYTES, MAX_IMAGES,
};
#[doc(inline)]
pub use digest::QuestionDigest;
#[doc(inline)]
pub use error::DecisionError;
#[doc(inline)]
pub use map::OrderedMap;
#[doc(inline)]
pub use models::{ModelMetadata, ModelMetadataList};
#[doc(inline)]
pub use request::{Asset, AssetKind, ImageFormat, NoulCriteria, Question, State, SystemOneRequest};
#[doc(inline)]
pub use response::{Answer, BoxCoordinates, SystemOneResponse, Usage};
