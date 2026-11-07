//! Stable stdout bytes for a subjects document.

use sc_agent_types::JevError;
use sc_agent_types::schema::subjects::SubjectsDocument;

/// Serializes `document` as pretty JSON plus a trailing newline.
///
/// The CLI writes these bytes to stdout unchanged. Fixture tests compare them
/// to `subjects.json`.
///
/// # Errors
///
/// Returns [`JevError::ExtractionInvalid`] when `serde_json` cannot encode
/// `document`.
pub fn subjects_to_json_bytes(document: &SubjectsDocument) -> Result<Vec<u8>, JevError> {
    let mut bytes = serde_json::to_vec_pretty(document).map_err(|error| {
        JevError::extraction_invalid(format!("subjects JSON encoding failed: {error}"))
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}
