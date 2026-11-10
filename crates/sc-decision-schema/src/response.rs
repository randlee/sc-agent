//! The POST /v1/systemone response body: [`SystemOneResponse`] and its answers.
//!
//! This is the one response form of the decision layer (ADR-SAG-0001): every backend
//! decodes its provider's reply into it. Fields outside the TypeSafe schema, which other
//! System One servers add, are kept in `extra` so the reply round-trips whole. A reply parsed
//! by [`SystemOneResponse::from_json`] keeps its document's key order. Numbers keep their
//! value exactly; nothing is rounded (REQ-SAG-0007). A typed `f64` prints in its shortest
//! exact form (`0.123456789`, `1.0`), so a number's text can change (`0.50` comes back as
//! `0.5`) but never its value.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::DecisionError;
use crate::key_order::{self, KeyOrder, Ordered};
use crate::map::OrderedMap;

/// The POST /v1/systemone reply: one answer per question id, in request order.
///
/// The pub fields deliberately mirror the wire spec. A reply parsed by [`Self::from_json`]
/// serialises back in its document's key order; one built by [`Self::new`] in schema order.
/// Equality compares the fields only, not the key order.
///
/// Parsing directly with serde (`serde_json::from_str`, `serde_json::from_slice`,
/// `serde_json::from_value` and the like) does not keep the document's key order: the value
/// serialises in schema order. `from_json` is the entry point for byte-identical round
/// trips (REQ-SAG-0201 AC1).
///
/// # Examples
///
/// ```
/// use sc_decision_schema::{Answer, SystemOneResponse};
///
/// let body = br#"{"model":"jev-1","answers":{"q1":{"type":"noul","noul":0.123456789}},"usage":{"input_tokens":12,"output_tokens":1}}"#;
/// let response = SystemOneResponse::from_json(200, body)?;
/// let (_, answer) = response.answers.iter().next().expect("one answer");
/// assert_eq!(answer, &Answer::Noul { noul: 0.123456789 });
/// assert_eq!(serde_json::to_vec(&response).expect("encode"), body.to_vec());
/// # Ok::<(), sc_decision_schema::DecisionError>(())
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(remote = "Self")]
pub struct SystemOneResponse {
    /// The model that answered.
    pub model: String,
    /// The answers by question id.
    pub answers: OrderedMap<String, Answer>,
    /// Token counts.
    pub usage: Usage,
    /// Fields outside the TypeSafe schema (Strands Decider `latency_ms`, Laya `routing`), kept verbatim.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
    /// The key order of the parsed document; empty for a built reply.
    #[serde(skip)]
    key_order: KeyOrder,
}

impl SystemOneResponse {
    /// A reply of these fields; it serialises in schema order.
    #[must_use]
    pub fn new(
        model: impl Into<String>,
        answers: OrderedMap<String, Answer>,
        usage: Usage,
        extra: serde_json::Map<String, serde_json::Value>,
    ) -> Self {
        Self {
            model: model.into(),
            answers,
            usage,
            extra,
            key_order: KeyOrder::default(),
        }
    }

    /// Parses a reply body of HTTP `status`, keeping its key order.
    ///
    /// Only parses: the caller decides which statuses carry a response.
    ///
    /// # Errors
    ///
    /// [`DecisionError::ResponseParse`] with `status` and the serde_json message (with the
    /// field path when it is not the root) when `body` is not a `SystemOneResponse`.
    pub fn from_json(status: u16, body: &[u8]) -> Result<Self, DecisionError> {
        key_order::parse(body).map_err(|error| DecisionError::ResponseParse {
            status,
            expected: "SystemOneResponse",
            message: error.located(),
        })
    }
}

impl PartialEq for SystemOneResponse {
    fn eq(&self, other: &Self) -> bool {
        self.model == other.model
            && self.answers == other.answers
            && self.usage == other.usage
            && self.extra == other.extra
    }
}

impl Ordered for SystemOneResponse {
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

impl Serialize for SystemOneResponse {
    /// Writes the document's key order (schema order when built).
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        key_order::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for SystemOneResponse {
    /// Parses in schema order without recording the key order; [`Self::from_json`] keeps it.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::deserialize(deserializer)
    }
}

/// Token counts of one reply; extra counters (Laya `truncated`) are kept in `extra`.
///
/// The pub fields deliberately mirror the wire spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    /// Tokens read.
    pub input_tokens: u64,
    /// Tokens written.
    pub output_tokens: u64,
    /// Fields outside the TypeSafe schema, kept verbatim.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// One answer, discriminated by `type` like its [`Question`](crate::Question).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Answer {
    /// The probability of yes.
    Noul {
        /// The probability of yes, 0 to 1.
        noul: f64,
    },
    /// The picked option.
    Choice {
        /// The picked option name.
        choice: String,
        /// The confidence in the pick.
        confidence: f64,
        /// The probability of each option, in criteria order.
        probabilities: OrderedMap<String, f64>,
    },
    /// The score on the question's levels.
    Score {
        /// The score.
        score: f64,
        /// The confidence in the score.
        confidence: f64,
        /// The levels by number, as the question gave them.
        legend: OrderedMap<String, serde_json::Value>,
        /// The probability of each level.
        probabilities: OrderedMap<String, f64>,
    },
    /// The located region.
    Box {
        /// The region.
        #[serde(rename = "box")]
        coordinates: BoxCoordinates,
        /// The confidence in the region; required on the wire, may be null.
        #[serde(deserialize_with = "Option::deserialize")]
        confidence: Option<f64>,
    },
}

/// The edges of a box answer.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoxCoordinates {
    /// The top edge.
    pub top: f64,
    /// The bottom edge.
    pub bottom: f64,
    /// The left edge.
    pub left: f64,
    /// The right edge.
    pub right: f64,
}
