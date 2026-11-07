//! Versioned stdout envelope (ADR-SAG-0014, ADR-SAG-0024).
//!
//! The envelope type and its operation names live in
//! [`sc_jev_check::run`]; this module only encodes it as one stdout line.

use sc_agent_types::JevError;
use sc_jev_check::run::Envelope;

#[doc(inline)]
pub use sc_jev_check::run::{ENVELOPE_VERSION, OP_ASK, OP_DRY_RUN, OP_RUN};

/// `envelope` as compact JSON plus a trailing newline.
pub(crate) fn to_bytes(envelope: &Envelope) -> Result<Vec<u8>, JevError> {
    let mut bytes = serde_json::to_vec(envelope)
        .map_err(|error| JevError::encode_failed("response JSON", &error))?;
    bytes.push(b'\n');
    Ok(bytes)
}
