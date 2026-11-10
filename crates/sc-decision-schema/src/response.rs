//! The POST /v1/systemone response body: [`SystemOneResponse`] and its answers.
//!
//! This is the one response form of the decision layer (ADR-SAG-0001): every backend
//! decodes its provider's reply into it. Fields outside the TypeSafe schema, which other
//! System One servers add, are kept verbatim in `extra` so the reply round-trips whole.
//! Numbers are kept exactly as sent; nothing is rounded (REQ-SAG-0007).

use serde::{Deserialize, Serialize};

use crate::DecisionError;
use crate::map::OrderedMap;

/// The POST /v1/systemone reply: one answer per question id, in request order.
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
}

impl SystemOneResponse {
    /// Parses a reply body of HTTP `status`.
    ///
    /// Only parses: the caller decides which statuses carry a response.
    ///
    /// # Errors
    ///
    /// [`DecisionError::ResponseParse`] with `status` and the serde_json message when `body`
    /// is not a `SystemOneResponse`.
    pub fn from_json(status: u16, body: &[u8]) -> Result<Self, DecisionError> {
        serde_json::from_slice(body).map_err(|error| DecisionError::ResponseParse {
            status,
            expected: "SystemOneResponse",
            message: error.to_string(),
        })
    }
}

/// Token counts of one reply; extra counters (Laya `truncated`) are kept in `extra`.
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
