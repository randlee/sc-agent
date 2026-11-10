#![warn(missing_docs)]

mod extract;
mod grammar;
mod query;
mod render;

#[doc(inline)]
pub use extract::ExtractInput;
#[doc(inline)]
pub use extract::extract;
#[doc(inline)]
pub use grammar::LanguageInfo;
#[doc(inline)]
pub use grammar::language;
#[doc(inline)]
pub use grammar::languages;
#[doc(inline)]
pub use query::QueryInfo;
#[doc(inline)]
pub use query::queries;
#[doc(inline)]
pub use query::query_source;
#[doc(inline)]
pub use render::subjects_to_json_bytes;

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
