//! Versioned stdout envelope (ADR-SAG-0014).

use sc_agent_types::JevError;
use serde_json::Value;

/// Envelope `version` field.
pub const ENVELOPE_VERSION: &str = "1";

/// `sc-ai jev run` operation name.
pub const OP_RUN: &str = "jev.run";

/// `sc-ai jev dry-run` operation name.
pub const OP_DRY_RUN: &str = "jev.dry-run";

/// `sc-ai jev ask` operation name.
pub const OP_ASK: &str = "jev.ask";

pub(crate) fn envelope(operation: &str, data: Value) -> Result<Vec<u8>, JevError> {
    let value = serde_json::json!({
        "version": ENVELOPE_VERSION,
        "operation": operation,
        "data": data,
    });
    let mut bytes = serde_json::to_vec(&value).map_err(|_| JevError::InputInvalid {
        detail: "response JSON could not be encoded".into(),
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}
