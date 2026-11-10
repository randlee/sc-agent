//! The POST /v1/systemone request body: [`SystemOneRequest`] and its parts.
//!
//! Shapes follow the TypeSafe OpenAPI document (v0.2.0). A request parsed by
//! [`SystemOneRequest::from_json`] keeps its document's key order, so a documented example
//! serialises back byte-identical; values inside `serde_json::Value` fields (state,
//! instructions, criteria) keep their value, but a number's text may change (`1.50` comes
//! back as `1.5`). Unknown fields are refused: a typo in a request names its field instead
//! of being dropped.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::DecisionError;
use crate::key_order::{self, KeyOrder, Ordered};
use crate::map::OrderedMap;

/// The POST /v1/systemone body.
///
/// The pub fields deliberately mirror the wire spec (TypeSafe openapi.json v0.2.0): one field
/// per JSON property, read and set directly. A request parsed by [`Self::from_json`] keeps the
/// key order of its document, nested objects included, and serialises back in it; one built
/// by [`Self::new`] serialises in schema order (`model`, `state`, `questions`, `assets`).
/// Equality compares the fields only, not the key order.
///
/// # Examples
///
/// ```
/// use sc_decision_schema::SystemOneRequest;
///
/// let text = r#"{"model":"jev-1","state":"The sky is green.","questions":{"q1":{"type":"noul","instructions":"Is the statement true?"}}}"#;
/// let request = SystemOneRequest::from_json(text)?;
/// assert_eq!(request.model, "jev-1");
/// assert_eq!(request.to_json()?, text);
/// # Ok::<(), sc_decision_schema::DecisionError>(())
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct SystemOneRequest {
    /// The model name (GET /v1/models lists them).
    pub model: String,
    /// What the questions are asked about.
    pub state: State,
    /// The questions by id, in request order; the answers come back under the same ids.
    pub questions: OrderedMap<String, Question>,
    /// Images the state and questions refer to, by asset id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<OrderedMap<String, Asset>>,
    /// The key order of the parsed document; empty for a built request.
    #[serde(skip)]
    key_order: KeyOrder,
}

impl SystemOneRequest {
    /// A request of these fields; it serialises in schema order.
    #[must_use]
    pub fn new(
        model: impl Into<String>,
        state: State,
        questions: OrderedMap<String, Question>,
        assets: Option<OrderedMap<String, Asset>>,
    ) -> Self {
        Self {
            model: model.into(),
            state,
            questions,
            assets,
            key_order: KeyOrder::default(),
        }
    }

    /// Parses a request from JSON text, naming the failing field (REQ-SAG-0504).
    ///
    /// The document's key order is kept for [`Self::to_json`].
    ///
    /// # Errors
    ///
    /// [`DecisionError::RequestParse`] with the field path (`.` at the root, or the unknown
    /// field's name) and the full serde_json message, including line and column, when `text`
    /// is not a request.
    pub fn from_json(text: &str) -> Result<Self, DecisionError> {
        key_order::parse(text.as_bytes()).map_err(|error| DecisionError::RequestParse {
            path: error.path,
            message: error.message,
        })
    }

    /// The request as compact JSON, in its document's key order (schema order when built).
    ///
    /// # Errors
    ///
    /// [`DecisionError::Encode`] when serde_json refuses a value.
    pub fn to_json(&self) -> Result<String, DecisionError> {
        serde_json::to_string(self).map_err(|error| DecisionError::Encode {
            message: format!("the request does not serialise: {error}"),
        })
    }
}

impl PartialEq for SystemOneRequest {
    fn eq(&self, other: &Self) -> bool {
        self.model == other.model
            && self.state == other.state
            && self.questions == other.questions
            && self.assets == other.assets
    }
}

impl Ordered for SystemOneRequest {
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

impl Serialize for SystemOneRequest {
    /// Writes the document's key order (schema order when built).
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        key_order::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for SystemOneRequest {
    /// Parses in schema order without recording the key order; [`Self::from_json`] keeps it.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::deserialize(deserializer)
    }
}

/// What the questions are asked about: a string, an object or an array.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum State {
    /// Plain text.
    Text(String),
    /// A JSON object, key order kept.
    Object(serde_json::Map<String, serde_json::Value>),
    /// A JSON array.
    Array(Vec<serde_json::Value>),
}

/// One question, discriminated by `type`.
///
/// `instructions` and criteria values are a string, object, array or null, kept as JSON
/// values. An absent field and an explicit `null` both round-trip: absent is `None`, null is
/// `Some(Value::Null)` (or `Some(None)` for noul criteria).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum Question {
    /// A yes/no question; the answer is the probability of yes.
    Noul {
        /// How to answer.
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        instructions: Option<serde_json::Value>,
        /// What yes and no mean: absent, `null` (`Some(None)`) or `{true, false}`.
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        criteria: Option<Option<NoulCriteria>>,
    },
    /// Pick one of the named options.
    Choice {
        /// How to answer.
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        instructions: Option<serde_json::Value>,
        /// The options by name, in order.
        criteria: OrderedMap<String, serde_json::Value>,
    },
    /// Score on ordered levels.
    Score {
        /// How to answer.
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        instructions: Option<serde_json::Value>,
        /// The levels, in order; a level's number is its index from 0.
        criteria: Vec<serde_json::Value>,
    },
    /// Locate a region of an image.
    Box {
        /// How to answer.
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        instructions: Option<serde_json::Value>,
    },
}

/// What yes (`true`) and no (`false`) mean for a noul question.
///
/// The pub fields deliberately mirror the wire spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoulCriteria {
    /// The meaning of yes.
    #[serde(
        rename = "true",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub yes: Option<serde_json::Value>,
    /// The meaning of no.
    #[serde(
        rename = "false",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub no: Option<serde_json::Value>,
}

/// One image the request refers to: inline (`data_base64`) or a multipart part (`part`).
///
/// The pub fields deliberately mirror the wire spec; every property is optional in
/// openapi.json v0.2.0, `type` included.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    /// The asset kind (`"image"`, the only kind); absent when the document omits it.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<AssetKind>,
    /// The image format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<ImageFormat>,
    /// The image bytes, base64; opaque to this crate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_base64: Option<String>,
    /// The multipart part name that carries the image bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<String>,
}

/// The kind of an [`Asset`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum AssetKind {
    /// An image.
    Image,
}

/// The format of an image asset or part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ImageFormat {
    /// PNG.
    Png,
    /// JPEG, spelled `jpg`.
    Jpg,
    /// JPEG, spelled `jpeg`.
    Jpeg,
}

/// Keeps an explicit JSON null as `Some(Null)` so absent and null both round-trip.
///
/// Used with `#[serde(default)]`: an absent field never reaches this function and stays
/// `None`; a present field, null included, becomes `Some`.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}
