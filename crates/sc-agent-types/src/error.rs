//! `JevError` is the only CLI and PyO3 error-id source (ADR-SAG-0001).
//!
//! Variant codes match `tests/fixtures/jev/README-contract/errors.json`.

use crate::ids::{BackendKeyId, SessionId};

/// Marks a [`JevError::ConfigInvalid`] detail that describes a credential store.
///
/// [`JevError::fix_hint`] returns the text after this prefix. Other
/// `CONFIG_INVALID` details keep the `check.toml` recovery text.
const CREDENTIAL_STORE_DETAIL_PREFIX: &str = "credential-store: ";

/// Stable error ids and fix text for Jev, check, extraction, and the CLI.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JevError {
    /// `check.toml` or a packaged asset failed validation.
    #[error("config is invalid: {detail}")]
    ConfigInvalid {
        /// What failed validation.
        detail: String,
    },
    /// No credential was found for a backend.
    #[error("API key for {backend} is missing")]
    KeyMissing {
        /// Backend whose credential is missing.
        backend: BackendKeyId,
    },
    /// The check input exceeds the single-request byte cap.
    #[error("input is {actual_bytes} bytes; limit is {limit_bytes}")]
    InputTooLarge {
        /// Maximum accepted payload size.
        limit_bytes: u64,
        /// Measured input size.
        actual_bytes: u64,
    },
    /// The check input contained no usable content.
    #[error("input is empty")]
    InputEmpty,
    /// The check input did not match the expected shape.
    #[error("input is invalid: {detail}")]
    InputInvalid {
        /// What about the input is wrong.
        detail: String,
    },
    /// Extraction succeeded and produced zero subjects.
    #[error("extraction produced no subjects")]
    ExtractionEmpty,
    /// A subject span contains a parse error.
    #[error("parse error in {path}:{line}")]
    ExtractionParseError {
        /// Source path that failed to parse.
        path: String,
        /// 1-based line of the parse error.
        line: u32,
    },
    /// Extractor stdout was not a valid subjects document.
    #[error("extraction output is invalid: {detail}")]
    ExtractionInvalid {
        /// What about the artifact is wrong.
        detail: String,
    },
    /// The extractor process failed.
    #[error("extraction failed: {detail}")]
    ExtractionFailed {
        /// Producer failure detail.
        detail: String,
    },
    /// The extractor exceeded its time limit.
    #[error("extraction timed out after {timeout_seconds}s")]
    ExtractionTimeout {
        /// Limit that was exceeded.
        timeout_seconds: u64,
    },
    /// The requested grammar is not built into `sc-treesitter`.
    #[error("language {language} is unavailable")]
    LanguageUnavailable {
        /// Requested language id.
        language: String,
    },
    /// A tree-sitter query failed to compile.
    #[error("query is invalid: {detail}")]
    QueryInvalid {
        /// Query compiler detail.
        detail: String,
    },
    /// The named query asset does not exist.
    #[error("query {query} was not found")]
    QueryNotFound {
        /// Query name that was requested.
        query: String,
    },
    /// An input path does not exist.
    #[error("input path {path} is missing")]
    InputMissing {
        /// Path that could not be read.
        path: String,
    },
    /// A judge response failed validation.
    #[error("response is invalid: {detail}")]
    ResponseInvalid {
        /// What about the response is wrong.
        detail: String,
    },
    /// A backend transport failed.
    #[error("network error: {detail}")]
    NetworkError {
        /// Transport failure detail, with secrets removed.
        detail: String,
    },
    /// An LLM system-prompt file could not be read.
    #[error("prompt file {path} is missing")]
    PromptFileMissing {
        /// Path that could not be read.
        path: String,
    },
    /// The session id does not exist in the store.
    #[error("session {session_id} was not found")]
    SessionNotFound {
        /// Id that was requested.
        session_id: SessionId,
    },
    /// The session is past its TTL.
    #[error("session {session_id} has expired")]
    SessionExpired {
        /// Id that expired.
        session_id: SessionId,
    },
}

impl JevError {
    /// Builds `CONFIG_INVALID` for an unreadable credential store.
    ///
    /// `what` names the store, such as the repo-root `.env` file or the
    /// platform keyring. `cause` is a sanitized label (an I/O kind or a
    /// keyring variant). It must not contain secret bytes, passwords, or the
    /// raw platform error text. `recovery` is the caller-facing fix.
    /// [`Self::fix_hint`] returns those three parts and does not mention
    /// `check.toml`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::JevError;
    ///
    /// let error = JevError::credential_store_invalid(
    ///     "platform keyring could not be read",
    ///     "keystore access was denied",
    ///     "Unlock the platform keystore for this OS user and retry.",
    /// );
    /// assert_eq!(error.code(), "CONFIG_INVALID");
    /// assert!(error.fix_hint().contains("keystore access was denied"));
    /// assert!(!error.fix_hint().contains("check.toml"));
    /// ```
    #[must_use]
    pub fn credential_store_invalid(what: &str, cause: &str, recovery: &str) -> Self {
        Self::ConfigInvalid {
            detail: format!("{CREDENTIAL_STORE_DETAIL_PREFIX}{what} (cause: {cause}). {recovery}"),
        }
    }

    /// Returns the stable machine-readable id for this error.
    ///
    /// Ids match `tests/fixtures/jev/README-contract/errors.json` and are the
    /// only values CLI and PyO3 may emit in the `error` field.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::ConfigInvalid { .. } => "CONFIG_INVALID",
            Self::KeyMissing { .. } => "KEY_MISSING",
            Self::InputTooLarge { .. } => "INPUT_TOO_LARGE",
            Self::InputEmpty => "INPUT_EMPTY",
            Self::InputInvalid { .. } => "INPUT_INVALID",
            Self::ExtractionEmpty => "EXTRACTION_EMPTY",
            Self::ExtractionParseError { .. } => "EXTRACTION_PARSE_ERROR",
            Self::ExtractionInvalid { .. } => "EXTRACTION_INVALID",
            Self::ExtractionFailed { .. } => "EXTRACTION_FAILED",
            Self::ExtractionTimeout { .. } => "EXTRACTION_TIMEOUT",
            Self::LanguageUnavailable { .. } => "LANGUAGE_UNAVAILABLE",
            Self::QueryInvalid { .. } => "QUERY_INVALID",
            Self::QueryNotFound { .. } => "QUERY_NOT_FOUND",
            Self::InputMissing { .. } => "INPUT_MISSING",
            Self::ResponseInvalid { .. } => "RESPONSE_INVALID",
            Self::NetworkError { .. } => "NETWORK_ERROR",
            Self::PromptFileMissing { .. } => "PROMPT_FILE_MISSING",
            Self::SessionNotFound { .. } => "SESSION_NOT_FOUND",
            Self::SessionExpired { .. } => "SESSION_EXPIRED",
        }
    }

    /// Returns the actionable fix text for this error.
    ///
    /// The CLI copies this string into the stderr `message` field. Variants
    /// that carry `detail` or `session_id` include that value. Callers must
    /// keep secrets out of `detail`.
    ///
    /// Credential-store failures built with [`Self::credential_store_invalid`]
    /// return the store name, sanitized cause, and recovery text. Other
    /// `CONFIG_INVALID` values keep the `check.toml` hint.
    #[must_use]
    pub fn fix_hint(&self) -> String {
        match self {
            Self::ConfigInvalid { detail } => {
                if let Some(message) = detail.strip_prefix(CREDENTIAL_STORE_DETAIL_PREFIX) {
                    message.to_owned()
                } else {
                    format!(
                        "Fix the check.toml field or packaged jev.toml entry ({detail}), then retry."
                    )
                }
            }
            Self::KeyMissing { backend } => format!(
                "Provide a credential for {backend} via the process environment, the repo-root .env file, or the keyring, then retry."
            ),
            Self::InputTooLarge {
                limit_bytes,
                actual_bytes,
            } => format!(
                "Shrink the check input from {actual_bytes} bytes to at most {limit_bytes} bytes. One check is one request and is never split."
            ),
            Self::InputEmpty => "Provide a non-empty input and retry.".to_owned(),
            Self::InputInvalid { detail } => format!(
                "Correct the input so it matches the expected schema ({detail}), then retry."
            ),
            Self::ExtractionEmpty => {
                "Change the query or input so extraction emits at least one subject.".to_owned()
            }
            Self::ExtractionParseError { path, line } => {
                format!("Fix the syntax error at {path}:{line} and run extraction again.")
            }
            Self::ExtractionInvalid { detail } => {
                format!("Emit a valid sc/subjects/1 document on stdout ({detail}) and retry.")
            }
            Self::ExtractionFailed { detail } => {
                format!("Fix the producer failure ({detail}) and run extraction again.")
            }
            Self::ExtractionTimeout { timeout_seconds } => format!(
                "Raise timeout_seconds above {timeout_seconds} or reduce the input so extraction finishes in time."
            ),
            Self::LanguageUnavailable { language } => {
                format!("Use a language built into sc-treesitter instead of {language}.")
            }
            Self::QueryInvalid { detail } => {
                format!("Fix the tree-sitter query syntax ({detail}) and retry.")
            }
            Self::QueryNotFound { query } => {
                format!("Choose a query asset that exists for this language instead of {query}.")
            }
            Self::InputMissing { path } => {
                format!("Create {path} or correct the path in check.toml, then retry.")
            }
            Self::ResponseInvalid { detail } => format!(
                "Retry the request. The judge response was invalid ({detail}). If it persists, treat a truncated or off-schema response as a backend failure."
            ),
            Self::NetworkError { detail } => format!(
                "Check connectivity and retry. Transport detail: {detail}. Backends retry busy responses before returning this error."
            ),
            Self::PromptFileMissing { path } => format!(
                "Provide the LLM system prompt at {path}, or select it with check.toml or the CLI override."
            ),
            Self::SessionNotFound { session_id } => format!(
                "Create a session or pass the id of a session that still exists. Missing session: {session_id}."
            ),
            Self::SessionExpired { session_id } => format!(
                "Start a new session. Session {session_id} expired; expiry is measured from the session created_at timestamp."
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use uuid::Uuid;

    fn samples() -> Vec<JevError> {
        let session = SessionId::from_uuid(Uuid::nil());
        vec![
            JevError::ConfigInvalid {
                detail: "missing model".into(),
            },
            JevError::KeyMissing {
                backend: BackendKeyId::TypeSafe,
            },
            JevError::InputTooLarge {
                limit_bytes: 10,
                actual_bytes: 11,
            },
            JevError::InputEmpty,
            JevError::InputInvalid {
                detail: "bad json".into(),
            },
            JevError::ExtractionEmpty,
            JevError::ExtractionParseError {
                path: "a.py".into(),
                line: 3,
            },
            JevError::ExtractionInvalid {
                detail: "schema".into(),
            },
            JevError::ExtractionFailed {
                detail: "exit 1".into(),
            },
            JevError::ExtractionTimeout {
                timeout_seconds: 30,
            },
            JevError::LanguageUnavailable {
                language: "go".into(),
            },
            JevError::QueryInvalid {
                detail: "syntax".into(),
            },
            JevError::QueryNotFound {
                query: "missing".into(),
            },
            JevError::InputMissing {
                path: "gone.toml".into(),
            },
            JevError::ResponseInvalid {
                detail: "truncated".into(),
            },
            JevError::NetworkError {
                detail: "reset".into(),
            },
            JevError::PromptFileMissing {
                path: "prompt.md".into(),
            },
            JevError::SessionNotFound {
                session_id: session,
            },
            JevError::SessionExpired {
                session_id: session,
            },
        ]
    }

    #[test]
    fn codes_match_readme_contract_snapshot() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/jev/README-contract/errors.json");
        let text = std::fs::read_to_string(&path).expect("read errors.json");
        let document: serde_json::Value = serde_json::from_str(&text).expect("parse errors.json");
        let expected: BTreeSet<String> = document["error_ids"]
            .as_array()
            .expect("error_ids array")
            .iter()
            .map(|value| value.as_str().expect("error id string").to_owned())
            .collect();
        let actual: BTreeSet<String> = samples()
            .iter()
            .map(JevError::code)
            .map(str::to_owned)
            .collect();
        assert_eq!(actual, expected);
        assert_eq!(actual.len(), samples().len(), "error codes must be unique");
    }

    #[test]
    fn every_variant_has_a_fix_hint() {
        for error in samples() {
            assert!(!error.fix_hint().trim().is_empty(), "{}", error.code());
            assert!(!error.code().is_empty());
        }
    }

    #[test]
    fn fix_hints_include_detail_and_session_id() {
        let session = SessionId::from_uuid(Uuid::nil());
        let detailed = [
            JevError::ConfigInvalid {
                detail: "missing model".into(),
            },
            JevError::InputInvalid {
                detail: "bad json".into(),
            },
            JevError::ExtractionInvalid {
                detail: "schema".into(),
            },
            JevError::ExtractionFailed {
                detail: "exit 1".into(),
            },
            JevError::QueryInvalid {
                detail: "syntax".into(),
            },
            JevError::ResponseInvalid {
                detail: "truncated".into(),
            },
            JevError::NetworkError {
                detail: "connection reset".into(),
            },
        ];
        let needles = [
            "missing model",
            "bad json",
            "schema",
            "exit 1",
            "syntax",
            "truncated",
            "connection reset",
        ];
        for (error, needle) in detailed.iter().zip(needles) {
            let hint = error.fix_hint();
            assert!(
                hint.contains(needle),
                "{} hint {hint:?} missing {needle}",
                error.code()
            );
            if error.code() == "CONFIG_INVALID" {
                assert!(
                    hint.contains("check.toml"),
                    "non-credential CONFIG_INVALID keeps the check.toml hint: {hint}"
                );
            }
            assert!(
                !hint.to_ascii_lowercase().contains("bearer "),
                "{} hint must not add credential material",
                error.code()
            );
        }
        let too_large = JevError::InputTooLarge {
            limit_bytes: 10,
            actual_bytes: 11,
        };
        assert!(too_large.fix_hint().contains("11"));
        assert!(too_large.fix_hint().contains("10"));
        let session_text = session.to_string();
        assert!(
            JevError::SessionNotFound {
                session_id: session,
            }
            .fix_hint()
            .contains(&session_text)
        );
        assert!(
            JevError::SessionExpired {
                session_id: session,
            }
            .fix_hint()
            .contains(&session_text)
        );
    }
}
