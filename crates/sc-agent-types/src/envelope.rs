//! CLI stderr JSON shape (NFR-SAG-0001).
//!
//! Success envelopes stay in `sc-ai`. This crate only defines the error body
//! so every CLI crate copies ids from [`crate::JevError`].

use serde::Serialize;

use crate::error::JevError;

/// stderr body for a failed CLI command.
///
/// Serialized form is `{"error":"<code>","message":"<fix hint>"}`.
/// Fields stay private so the only Rust constructor is [`Self::from_jev_error`].
/// Neither field holds a secret (NFR-SAG-0002).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScAgentErrorResponse {
    /// [`JevError::code`] value.
    error: String,
    /// [`JevError::fix_hint`] text.
    message: String,
}

impl ScAgentErrorResponse {
    /// Copies the stable code and fix hint from `error`.
    ///
    /// This is the only constructor. The hint is whatever [`JevError::fix_hint`]
    /// returns, including safe `detail` and `session_id` text.
    #[must_use]
    pub fn from_jev_error(error: &JevError) -> Self {
        Self {
            error: error.code().to_owned(),
            message: error.fix_hint(),
        }
    }

    /// Returns the stable error id.
    #[must_use]
    pub fn error(&self) -> &str {
        &self.error
    }

    /// Returns the fix hint copied from [`JevError::fix_hint`].
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
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
        assert_eq!(body.error(), "INPUT_EMPTY");
        assert_eq!(body.message(), JevError::InputEmpty.fix_hint());
    }

    #[test]
    fn message_includes_safe_detail_and_no_secret_field() {
        let error = JevError::ConfigInvalid {
            detail: "missing model".into(),
        };
        let body = ScAgentErrorResponse::from_jev_error(&error);
        assert!(body.message().contains("missing model"));
        let value = serde_json::to_value(&body).expect("serialize");
        let keys: Vec<&str> = value
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, ["error", "message"]);
        for key in keys {
            let lower = key.to_ascii_lowercase();
            for forbidden in ["secret", "token", "password", "authorization", "credential"] {
                assert!(!lower.contains(forbidden), "{key}");
            }
        }
    }
}
