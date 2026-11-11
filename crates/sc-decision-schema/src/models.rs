//! The GET /v1/models reply: [`ModelMetadataList`].

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::DecisionError;
use crate::key_order::{self, KeyOrder, Ordered};

/// The models a System One server offers.
///
/// The pub field deliberately mirrors the wire spec. A list parsed by [`Self::from_json`]
/// serialises back in its document's key order. Equality ignores the key order.
///
/// Parsing directly with serde (`serde_json::from_str`, `serde_json::from_slice`,
/// `serde_json::from_value` and the like) does not keep the document's key order: the value
/// serialises in schema order. `from_json` is the entry point for byte-identical round
/// trips (REQ-SAG-0201 AC1).
///
/// # Examples
///
/// ```
/// use sc_decision_schema::ModelMetadataList;
///
/// let body = br#"{"models":[{"name":"jev-1","description":"Jev","release_date":"2026-01-01"}]}"#;
/// let list = ModelMetadataList::from_json(200, body)?;
/// assert_eq!(list.models.first().map(|model| model.name.as_str()), Some("jev-1"));
/// # Ok::<(), sc_decision_schema::DecisionError>(())
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(remote = "Self")]
pub struct ModelMetadataList {
    /// The models, in the server's order.
    pub models: Vec<ModelMetadata>,
    /// The key order of the parsed document; empty for a built list.
    #[serde(skip)]
    key_order: KeyOrder,
}

/// One model of a [`ModelMetadataList`].
///
/// The pub fields deliberately mirror the wire spec.
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
    /// A list of `models`; it serialises in schema order.
    #[must_use]
    pub fn new(models: Vec<ModelMetadata>) -> Self {
        Self {
            models,
            key_order: KeyOrder::default(),
        }
    }

    /// Parses a GET /v1/models reply body of HTTP `status`, keeping its key order.
    ///
    /// # Errors
    ///
    /// [`DecisionError::ResponseParse`] with `status` and the serde_json message when `body`
    /// is not a `ModelMetadataList`.
    pub fn from_json(status: u16, body: &[u8]) -> Result<Self, DecisionError> {
        key_order::parse(body).map_err(|error| DecisionError::ResponseParse {
            status,
            expected: "ModelMetadataList",
            message: error.located(),
        })
    }
}

impl PartialEq for ModelMetadataList {
    fn eq(&self, other: &Self) -> bool {
        self.models == other.models
    }
}

impl Eq for ModelMetadataList {}

impl Ordered for ModelMetadataList {
    fn wire_deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::deserialize(deserializer)
    }

    fn wire_serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Self::serialize(self, serializer)
    }

    fn key_order(&self) -> &KeyOrder {
        &self.key_order
    }

    fn set_key_order(&mut self, order: KeyOrder) {
        self.key_order = order;
    }
}

impl Serialize for ModelMetadataList {
    /// Writes the document's key order (schema order when built).
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        key_order::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for ModelMetadataList {
    /// Parses in schema order without recording the key order; [`Self::from_json`] keeps it.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::deserialize(deserializer)
    }
}
