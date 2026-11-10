//! The documented error bodies of the TypeSafe API: HTTP 400 and HTTP 422.

use serde::{Deserialize, Serialize};

/// The HTTP 400 body: `detail` is a string or an object. The pub field mirrors the wire spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HttpBadRequestError {
    /// What the server refused, as it wrote it.
    pub detail: serde_json::Value,
}

/// The HTTP 422 body: one entry per field the server refused. The pub field mirrors the wire spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HttpValidationError {
    /// The refused fields.
    #[serde(default)]
    pub detail: Vec<ValidationError>,
}

/// One refused field of an [`HttpValidationError`]. The pub fields mirror the wire spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationError {
    /// Where the field is: names and array indexes from the body root.
    pub loc: Vec<LocItem>,
    /// What is wrong.
    pub msg: String,
    /// The server's error type.
    #[serde(rename = "type")]
    pub kind: String,
    /// The refused input, when the server returns it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    /// Extra context, when the server returns it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ctx: Option<serde_json::Map<String, serde_json::Value>>,
}

/// One step of a [`ValidationError::loc`]: a field name or an array index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum LocItem {
    /// A field name.
    Name(String),
    /// An array index.
    Index(u64),
}
