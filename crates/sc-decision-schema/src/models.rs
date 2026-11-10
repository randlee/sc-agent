//! The GET /v1/models reply: [`ModelMetadataList`].

use serde::{Deserialize, Serialize};

use crate::DecisionError;

/// The models a System One server offers.
///
/// # Examples
///
/// ```
/// use sc_decision_schema::ModelMetadataList;
///
/// let body = br#"{"models":[{"name":"jev-1","description":"Jev","release_date":"2026-01-01"}]}"#;
/// let list = ModelMetadataList::from_json(200, body)?;
/// assert_eq!(list.models[0].name, "jev-1");
/// # Ok::<(), sc_decision_schema::DecisionError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelMetadataList {
    /// The models, in the server's order.
    pub models: Vec<ModelMetadata>,
}

/// One model of a [`ModelMetadataList`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelMetadata {
    /// The name to put in `SystemOneRequest::model`.
    pub name: String,
    /// What the model is.
    pub description: String,
    /// The release date, as the server writes it.
    pub release_date: String,
}

impl ModelMetadataList {
    /// Parses a GET /v1/models reply body of HTTP `status`.
    ///
    /// # Errors
    ///
    /// [`DecisionError::ResponseParse`] with `status` and the serde_json message when `body`
    /// is not a `ModelMetadataList`.
    pub fn from_json(status: u16, body: &[u8]) -> Result<Self, DecisionError> {
        serde_json::from_slice(body).map_err(|error| DecisionError::ResponseParse {
            status,
            expected: "ModelMetadataList",
            message: error.to_string(),
        })
    }
}
