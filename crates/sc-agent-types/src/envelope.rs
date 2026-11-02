//! CLI stderr JSON shape (NFR-SAG-0001).
//!
//! Success envelopes stay in `sc-ai`. This crate only defines the error body
//! so every CLI crate copies ids from [`crate::JevError`].

use serde::{Deserialize, Serialize};

use crate::error::JevError;

/// stderr body for a failed CLI command.
///
/// Serialized form is `{"error":"<code>","message":"<fix hint>"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScAgentErrorResponse {
    /// [`JevError::code`] value.
    pub error: String,
    /// [`JevError::fix_hint`] text.
    pub message: String,
}

impl ScAgentErrorResponse {
    /// Copies the stable code and fix hint from `error`.
    #[must_use]
    pub fn from_jev_error(error: &JevError) -> Self {
        Self {
            error: error.code().to_owned(),
            message: error.fix_hint(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stderr_json_has_error_and_message_only() {
        let body = ScAgentErrorResponse::from_jev_error(&JevError::InputEmpty);
        let value = serde_json::to_value(&body).expect("serialize");
        let object = value.as_object().expect("object");
        assert_eq!(
            object.keys().cloned().collect::<Vec<_>>(),
            vec!["error".to_owned(), "message".to_owned()]
        );
        assert_eq!(body.error, "INPUT_EMPTY");
        assert_eq!(body.message, JevError::InputEmpty.fix_hint());
    }
}
