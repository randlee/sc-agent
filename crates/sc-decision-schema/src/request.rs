//! The POST /v1/systemone request body: [`SystemOneRequest`] and its parts.
//!
//! Shapes follow the TypeSafe OpenAPI document (v0.2.0). Field order is the wire order and
//! every object keeps its key order, so a parsed request serialises back byte-identical.
//! Unknown fields are refused: a typo in a request names its field instead of being dropped.

use serde::{Deserialize, Deserializer, Serialize};

use crate::DecisionError;
use crate::map::OrderedMap;

/// The POST /v1/systemone body. Field order is the wire order.
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
}

impl SystemOneRequest {
    /// Parses a request from JSON text, naming the failing field (REQ-SAG-0504).
    ///
    /// # Errors
    ///
    /// [`DecisionError::RequestParse`] with the field path (`.` at the root) and the full
    /// serde_json message, including line and column, when `text` is not a request.
    pub fn from_json(text: &str) -> Result<Self, DecisionError> {
        let mut deserializer = serde_json::Deserializer::from_str(text);
        let request = serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
            DecisionError::RequestParse {
                path: error.path().to_string(),
                message: error.inner().to_string(),
            }
        })?;
        deserializer
            .end()
            .map_err(|error| DecisionError::RequestParse {
                path: ".".to_owned(),
                message: error.to_string(),
            })?;
        Ok(request)
    }

    /// The request as compact JSON, in wire order.
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

/// What the questions are asked about: a string, an object or an array.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    /// The asset kind (`"image"`).
    #[serde(rename = "type")]
    pub kind: AssetKind,
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
pub enum AssetKind {
    /// An image.
    Image,
}

/// The format of an image asset or part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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
