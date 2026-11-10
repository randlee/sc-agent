//! The printed form of a result: one JSON line per `DecisionResult` (REQ-SAG-0701, REQ-SAG-0007).

use sc_agent_types::{ErrorDetail, ErrorId, SessionId};
use sc_decision_schema::{DecisionResult, OrderedMap, QuestionDigest, SystemOneResponse};

use crate::DecideError;

/// One printed result: its session ULID, input position, digests, and the response or the
/// error chain.
///
/// Numbers are written as serde_json gives them, never rounded (REQ-SAG-0007).
#[derive(Debug, serde::Serialize)]
pub struct ResultLine<'a> {
    /// The request's session, a ULID.
    pub session_id: &'a SessionId,
    /// The request's index in the input; 0 for a single request.
    pub position: u64,
    /// One digest per question.
    pub digests: &'a OrderedMap<String, QuestionDigest>,
    /// The decided response, when the decision succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<&'a SystemOneResponse>,
    /// `DecisionError::detail()`: every lower id, message and recovery.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorDetail>,
}

impl<'a> ResultLine<'a> {
    /// The line of `result`.
    #[must_use]
    pub fn new(result: &'a DecisionResult) -> Self {
        let (response, error) = match &result.outcome {
            Ok(response) => (Some(response), None),
            Err(error) => (None, Some(error.detail())),
        };
        Self {
            session_id: &result.session_id,
            position: result.position,
            digests: &result.digests,
            response,
            error,
        }
    }
}

/// `result` as one compact JSON object, without a trailing newline.
///
/// # Errors
///
/// [`DecideError::Encode`] when serde_json refuses a value.
pub fn result_line(result: &DecisionResult) -> Result<Vec<u8>, DecideError> {
    serde_json::to_vec(&ResultLine::new(result)).map_err(|error| DecideError::encode(&error))
}
