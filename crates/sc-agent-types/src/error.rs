//! `JevError` is the only CLI and PyO3 error-id source (ADR-SAG-0001).
//!
//! Every variant maps to exactly one id of
//! `tests/fixtures/jev/README-contract/errors.json`; an id may have several
//! variants (REQ-SAG-0101 as amended by r-1). Recovery categories are explicit
//! variants, and [`JevError::fix_hint`] is the one renderer of recovery text
//! (ADR-SAG-0027). No detail text is parsed for a prefix.
//!
//! # Construction
//!
//! The enum and every struct-like variant are `#[non_exhaustive]`, so other
//! crates build errors only through the snake-case constructors
//! ([`JevError::input_invalid`], [`JevError::network`], ...). Matching from
//! another crate needs `{ .. }` patterns and a wildcard arm.
//!
//! # Sources
//!
//! [`std::error::Error::source`] is `None` for every variant. The variants
//! that summarise a cause (`CredentialStoreInvalid`, `NetworkError`,
//! `EncodeFailed`, `IoFailed`, `SessionStoreIo`) keep only a redacted label
//! or an [`io::ErrorKind`], never the original error, so no unredacted text
//! can be reached through the source chain.

use std::fmt;
use std::io;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::envelope::ScAgentErrorResponse;
use crate::ids::{BackendKeyId, SessionId};

/// One stable error id of the README `errors.json` contract.
///
/// `JevError::code` returns this type. [`Self::ALL`] lists every id; the
/// `errors` integration test compares it with `errors.json`, and an
/// exhaustive in-crate test keeps [`Self::ALL`] complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum ErrorCode {
    /// `CONFIG_INVALID`.
    ConfigInvalid,
    /// `KEY_MISSING`.
    KeyMissing,
    /// `INPUT_TOO_LARGE`.
    InputTooLarge,
    /// `INPUT_EMPTY`.
    InputEmpty,
    /// `INPUT_INVALID`.
    InputInvalid,
    /// `EXTRACTION_EMPTY`.
    ExtractionEmpty,
    /// `EXTRACTION_PARSE_ERROR`.
    ExtractionParseError,
    /// `EXTRACTION_INVALID`.
    ExtractionInvalid,
    /// `EXTRACTION_FAILED`.
    ExtractionFailed,
    /// `EXTRACTION_TIMEOUT`.
    ExtractionTimeout,
    /// `LANGUAGE_UNAVAILABLE`.
    LanguageUnavailable,
    /// `QUERY_INVALID`.
    QueryInvalid,
    /// `QUERY_NOT_FOUND`.
    QueryNotFound,
    /// `INPUT_MISSING`.
    InputMissing,
    /// `RESPONSE_INVALID`.
    ResponseInvalid,
    /// `NETWORK_ERROR`.
    NetworkError,
    /// `PROMPT_FILE_MISSING`.
    PromptFileMissing,
    /// `SESSION_NOT_FOUND`.
    SessionNotFound,
    /// `SESSION_EXPIRED`.
    SessionExpired,
}

impl ErrorCode {
    /// Every error id, in `errors.json` order.
    pub const ALL: &'static [Self] = &[
        Self::ConfigInvalid,
        Self::KeyMissing,
        Self::InputTooLarge,
        Self::InputEmpty,
        Self::InputInvalid,
        Self::ExtractionEmpty,
        Self::ExtractionParseError,
        Self::ExtractionInvalid,
        Self::ExtractionFailed,
        Self::ExtractionTimeout,
        Self::LanguageUnavailable,
        Self::QueryInvalid,
        Self::QueryNotFound,
        Self::InputMissing,
        Self::ResponseInvalid,
        Self::NetworkError,
        Self::PromptFileMissing,
        Self::SessionNotFound,
        Self::SessionExpired,
    ];

    /// The stable id text, such as `INPUT_INVALID`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConfigInvalid => "CONFIG_INVALID",
            Self::KeyMissing => "KEY_MISSING",
            Self::InputTooLarge => "INPUT_TOO_LARGE",
            Self::InputEmpty => "INPUT_EMPTY",
            Self::InputInvalid => "INPUT_INVALID",
            Self::ExtractionEmpty => "EXTRACTION_EMPTY",
            Self::ExtractionParseError => "EXTRACTION_PARSE_ERROR",
            Self::ExtractionInvalid => "EXTRACTION_INVALID",
            Self::ExtractionFailed => "EXTRACTION_FAILED",
            Self::ExtractionTimeout => "EXTRACTION_TIMEOUT",
            Self::LanguageUnavailable => "LANGUAGE_UNAVAILABLE",
            Self::QueryInvalid => "QUERY_INVALID",
            Self::QueryNotFound => "QUERY_NOT_FOUND",
            Self::InputMissing => "INPUT_MISSING",
            Self::ResponseInvalid => "RESPONSE_INVALID",
            Self::NetworkError => "NETWORK_ERROR",
            Self::PromptFileMissing => "PROMPT_FILE_MISSING",
            Self::SessionNotFound => "SESSION_NOT_FOUND",
            Self::SessionExpired => "SESSION_EXPIRED",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ErrorCode {
    type Err = JevError;

    /// Parses one of the [`Self::ALL`] ids.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::ErrorCode;
    ///
    /// assert_eq!("INPUT_EMPTY".parse::<ErrorCode>().unwrap(), ErrorCode::InputEmpty);
    /// assert!("NOPE".parse::<ErrorCode>().is_err());
    /// ```
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .copied()
            .find(|code| code.as_str() == text)
            .ok_or_else(|| JevError::input_invalid(format!("{text:?} is not a stable error id")))
    }
}

impl PartialEq<str> for ErrorCode {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for ErrorCode {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<ErrorCode> for &str {
    fn eq(&self, other: &ErrorCode) -> bool {
        *self == other.as_str()
    }
}

impl AsRef<str> for ErrorCode {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Transport failure class of a [`JevError::NetworkError`].
///
/// The one spelling is snake_case for serde, `Display`, and `FromStr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum NetworkErrorKind {
    /// The provider said it is busy (rate limit or overload) after retries.
    Busy,
    /// The request timed out.
    Timeout,
    /// The connection could not be opened.
    Connect,
    /// The TLS handshake failed.
    Tls,
    /// The provider returned a non-success status.
    Status,
    /// The response body exceeded the read cap.
    BodyTooLarge,
    /// The caller cancelled the request.
    Cancelled,
}

impl NetworkErrorKind {
    /// Every kind, in declaration order.
    pub const ALL: &'static [Self] = &[
        Self::Busy,
        Self::Timeout,
        Self::Connect,
        Self::Tls,
        Self::Status,
        Self::BodyTooLarge,
        Self::Cancelled,
    ];

    /// The snake_case spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Busy => "busy",
            Self::Timeout => "timeout",
            Self::Connect => "connect",
            Self::Tls => "tls",
            Self::Status => "status",
            Self::BodyTooLarge => "body_too_large",
            Self::Cancelled => "cancelled",
        }
    }

    /// Whether a later retry may succeed (`Busy`, `Timeout`, `Connect`).
    #[must_use]
    pub const fn retryable(self) -> bool {
        matches!(self, Self::Busy | Self::Timeout | Self::Connect)
    }
}

impl fmt::Display for NetworkErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for NetworkErrorKind {
    type Err = JevError;

    /// Parses the snake_case spelling; anything else is `INPUT_INVALID`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .copied()
            .find(|kind| kind.as_str() == text)
            .ok_or_else(|| {
                JevError::input_invalid(format!(
                    "network error kind {text:?} is not busy, timeout, connect, tls, status, body_too_large, or cancelled"
                ))
            })
    }
}

/// What an operational failure was about; decides its error id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OperationalArea {
    /// A process, git, or log setup failure (`CONFIG_INVALID`).
    Config,
    /// A stdin or stdout failure (`INPUT_INVALID`).
    Io,
}

impl OperationalArea {
    /// The error id an operational failure in this area carries.
    #[must_use]
    pub const fn code(self) -> ErrorCode {
        match self {
            Self::Config => ErrorCode::ConfigInvalid,
            Self::Io => ErrorCode::InputInvalid,
        }
    }
}

/// Stable error ids and fix text for Jev, check, extraction, and the CLI.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum JevError {
    /// `check.toml` or a packaged asset failed validation.
    #[error("config is invalid: {detail}")]
    #[non_exhaustive]
    ConfigInvalid {
        /// What failed validation.
        detail: String,
    },
    /// An unreadable credential store (`CONFIG_INVALID`).
    #[error("credential store is invalid: {what} (cause: {cause})")]
    #[non_exhaustive]
    CredentialStoreInvalid {
        /// The store, such as the repo-root `.env` file.
        what: String,
        /// A sanitized cause label (an I/O kind or a keyring variant).
        cause: String,
        /// The caller-facing fix.
        recovery: String,
    },
    /// A backend-registry failure (`CONFIG_INVALID`).
    #[error("backend registry is invalid: {detail}")]
    #[non_exhaustive]
    RegistryInvalid {
        /// What is wrong, naming the backend id.
        detail: String,
        /// The caller-facing fix.
        recovery: String,
    },
    /// A session-store I/O failure (`CONFIG_INVALID`).
    #[error("session store {op} failed ({kind})")]
    #[non_exhaustive]
    SessionStoreIo {
        /// The operation and its target, such as `create session directory sessions`.
        op: String,
        /// The I/O error class; never the OS message.
        kind: io::ErrorKind,
        /// The caller-facing fix.
        recovery: String,
    },
    /// A process, git, log, stdin, or stdout failure; the id depends on `area`.
    #[error("operational failure: {recovery}")]
    #[non_exhaustive]
    Operational {
        /// `Config` (`CONFIG_INVALID`) or `Io` (`INPUT_INVALID`).
        area: OperationalArea,
        /// What failed and the next step.
        recovery: String,
    },
    /// A session file that is not a readable session document (`INPUT_INVALID`).
    #[error("session document is invalid: {detail}")]
    #[non_exhaustive]
    SessionDocumentInvalid {
        /// Which file and what is wrong.
        detail: String,
        /// The caller-facing fix.
        recovery: String,
    },
    /// A value `serde_json` could not encode or decode (`INPUT_INVALID`).
    #[error("{what} could not be encoded as JSON ({cause})")]
    #[non_exhaustive]
    EncodeFailed {
        /// What was being encoded, such as `response JSON`.
        what: String,
        /// The `serde_json` category and position only.
        cause: String,
    },
    /// A failed read or write (`INPUT_INVALID`).
    #[error("{what} I/O failed ({kind})")]
    #[non_exhaustive]
    IoFailed {
        /// The stream or file, such as `stdout`.
        what: String,
        /// The I/O error class; never the OS message.
        kind: io::ErrorKind,
    },
    /// No credential was found for a backend.
    #[error("API key for {backend} is missing")]
    #[non_exhaustive]
    KeyMissing {
        /// Backend whose credential is missing.
        backend: BackendKeyId,
    },
    /// The check input exceeds the single-request byte cap.
    #[error("input is {actual_bytes} bytes; limit is {limit_bytes}")]
    #[non_exhaustive]
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
    #[non_exhaustive]
    InputInvalid {
        /// What about the input is wrong.
        detail: String,
    },
    /// Extraction succeeded and produced zero subjects.
    #[error("extraction produced no subjects")]
    ExtractionEmpty,
    /// A subject span contains a parse error.
    #[error("parse error in {path}:{line}")]
    #[non_exhaustive]
    ExtractionParseError {
        /// Source path that failed to parse.
        path: String,
        /// 1-based line of the parse error.
        line: u32,
    },
    /// Extractor stdout was not a valid subjects document.
    #[error("extraction output is invalid: {detail}")]
    #[non_exhaustive]
    ExtractionInvalid {
        /// What about the artifact is wrong.
        detail: String,
    },
    /// The extractor process failed.
    #[error("extraction failed: {detail}")]
    #[non_exhaustive]
    ExtractionFailed {
        /// Producer failure detail.
        detail: String,
    },
    /// The extractor exceeded its time limit.
    #[error("extraction timed out after {timeout_seconds}s")]
    #[non_exhaustive]
    ExtractionTimeout {
        /// Limit that was exceeded.
        timeout_seconds: u64,
    },
    /// The requested grammar is not built into the producer.
    #[error("language {language} is unavailable in {producer}")]
    #[non_exhaustive]
    LanguageUnavailable {
        /// Producer command name, such as `sc-treesitter`.
        producer: String,
        /// Requested language id.
        language: String,
    },
    /// A producer query failed to compile.
    #[error("query is invalid: {detail}")]
    #[non_exhaustive]
    QueryInvalid {
        /// Producer command name.
        producer: String,
        /// Query compiler detail.
        detail: String,
    },
    /// The named query asset does not exist.
    #[error("query {query} was not found")]
    #[non_exhaustive]
    QueryNotFound {
        /// Producer command name.
        producer: String,
        /// Query name that was requested.
        query: String,
    },
    /// A producer reported a stable error on stderr; `code()` is the carried id.
    #[error("{producer} failed with {code}: {message}")]
    #[non_exhaustive]
    ProducerError {
        /// Producer command name from `check.toml`.
        producer: String,
        /// The producer's stable error id.
        code: ErrorCode,
        /// The producer's fix hint.
        message: String,
    },
    /// An input path does not exist.
    #[error("input path {path} is missing")]
    #[non_exhaustive]
    InputMissing {
        /// Path that could not be read.
        path: String,
    },
    /// A judge response failed validation.
    #[error("response is invalid: {detail}")]
    #[non_exhaustive]
    ResponseInvalid {
        /// What about the response is wrong.
        detail: String,
    },
    /// A backend transport failed.
    #[error("network error: {detail}")]
    #[non_exhaustive]
    NetworkError {
        /// Failure class; decides [`JevError::retryable`].
        kind: NetworkErrorKind,
        /// Transport failure detail, with secrets removed.
        detail: String,
    },
    /// An LLM system-prompt file could not be read.
    #[error("prompt file {path} is missing")]
    #[non_exhaustive]
    PromptFileMissing {
        /// Path that could not be read.
        path: String,
    },
    /// The session id does not exist in the store.
    #[error("session {session_id} was not found")]
    #[non_exhaustive]
    SessionNotFound {
        /// Id that was requested.
        session_id: SessionId,
    },
    /// The session is past its TTL.
    #[error("session {session_id} has expired")]
    #[non_exhaustive]
    SessionExpired {
        /// Id that expired.
        session_id: SessionId,
    },
}

impl JevError {
    /// `CONFIG_INVALID` for a `check.toml` or packaged-asset problem.
    #[must_use]
    pub fn config_invalid(detail: impl Into<String>) -> Self {
        Self::ConfigInvalid {
            detail: detail.into(),
        }
    }

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
    pub fn credential_store_invalid(
        what: impl Into<String>,
        cause: impl Into<String>,
        recovery: impl Into<String>,
    ) -> Self {
        Self::CredentialStoreInvalid {
            what: what.into(),
            cause: cause.into(),
            recovery: recovery.into(),
        }
    }

    /// Builds `CONFIG_INVALID` for a backend-registry failure.
    ///
    /// `detail` says what is wrong and names the backend id; `recovery` is
    /// the next step. [`Self::fix_hint`] returns `"{detail}. {recovery}"`
    /// and does not mention `check.toml`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::JevError;
    ///
    /// let error = JevError::registry_invalid(
    ///     "Backend jev is not registered",
    ///     "Register it before the run.",
    /// );
    /// assert_eq!(error.code(), "CONFIG_INVALID");
    /// assert_eq!(error.fix_hint(), "Backend jev is not registered. Register it before the run.");
    /// ```
    #[must_use]
    pub fn registry_invalid(detail: impl Into<String>, recovery: impl Into<String>) -> Self {
        Self::RegistryInvalid {
            detail: detail.into(),
            recovery: recovery.into(),
        }
    }

    /// Builds `CONFIG_INVALID` for a session-store I/O failure.
    ///
    /// `op` names the operation and its target, `kind` is the I/O class, and
    /// `recovery` is the next step. [`Self::fix_hint`] returns
    /// `"{op} failed (cause: {kind}). {recovery}"`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::JevError;
    ///
    /// let error = JevError::session_store_io(
    ///     "Create session directory sessions",
    ///     std::io::ErrorKind::PermissionDenied,
    ///     "Choose a writable directory.",
    /// );
    /// assert_eq!(error.code(), "CONFIG_INVALID");
    /// assert!(error.fix_hint().contains("permission denied"));
    /// assert!(!error.fix_hint().contains("check.toml"));
    /// ```
    #[must_use]
    pub fn session_store_io(
        op: impl Into<String>,
        kind: io::ErrorKind,
        recovery: impl Into<String>,
    ) -> Self {
        Self::SessionStoreIo {
            op: op.into(),
            kind,
            recovery: recovery.into(),
        }
    }

    /// Builds `CONFIG_INVALID` for a process, git, or log failure.
    ///
    /// `recovery` names what failed and the next step. [`Self::fix_hint`]
    /// returns `recovery` and does not mention `check.toml`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::JevError;
    ///
    /// let error = JevError::operational_config(
    ///     "Could not find a git toplevel. Pass --repo-root with the repository root.",
    /// );
    /// assert_eq!(error.code(), "CONFIG_INVALID");
    /// assert!(error.fix_hint().contains("--repo-root"));
    /// ```
    #[must_use]
    pub fn operational_config(recovery: impl Into<String>) -> Self {
        Self::Operational {
            area: OperationalArea::Config,
            recovery: recovery.into(),
        }
    }

    /// Builds `INPUT_INVALID` for stdin or stdout I/O.
    ///
    /// `recovery` names the stream and the next step. [`Self::fix_hint`]
    /// returns `recovery` and does not tell the caller to fix a check schema.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::JevError;
    ///
    /// let error = JevError::operational_io(
    ///     "stdin could not be read (broken pipe). Retry or pass --request-json as a file path.",
    /// );
    /// assert_eq!(error.code(), "INPUT_INVALID");
    /// assert!(!error.fix_hint().contains("schema"));
    /// ```
    #[must_use]
    pub fn operational_io(recovery: impl Into<String>) -> Self {
        Self::Operational {
            area: OperationalArea::Io,
            recovery: recovery.into(),
        }
    }

    /// Builds `INPUT_INVALID` for a session file that cannot be parsed.
    ///
    /// `detail` names the file and the problem; [`Self::fix_hint`] returns
    /// `"{detail}. {recovery}"`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::JevError;
    ///
    /// let error = JevError::session_document_invalid(
    ///     "Session file sessions/abc is not a session document",
    ///     "Delete it and start a new session.",
    /// );
    /// assert_eq!(error.code(), "INPUT_INVALID");
    /// assert!(error.fix_hint().contains("sessions/abc"));
    /// ```
    #[must_use]
    pub fn session_document_invalid(
        detail: impl Into<String>,
        recovery: impl Into<String>,
    ) -> Self {
        Self::SessionDocumentInvalid {
            detail: detail.into(),
            recovery: recovery.into(),
        }
    }

    /// Builds `INPUT_INVALID` for a value that `serde_json` could not encode
    /// or decode.
    ///
    /// `what` names what was being encoded, such as `"response JSON"`. Only
    /// the `serde_json` error category and the line and column are kept,
    /// never the document or the `serde_json` message, so no payload or
    /// secret text can leak through it.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::JevError;
    ///
    /// let cause = serde_json::from_str::<serde_json::Value>("{\"secret\": ").unwrap_err();
    /// let error = JevError::encode_failed("response JSON", &cause);
    /// assert_eq!(error.code(), "INPUT_INVALID");
    /// assert!(error.fix_hint().contains("response JSON"));
    /// assert!(!error.fix_hint().contains("secret"));
    /// ```
    #[must_use]
    pub fn encode_failed(what: impl Into<String>, error: &serde_json::Error) -> Self {
        let category = match error.classify() {
            serde_json::error::Category::Io => "I/O",
            serde_json::error::Category::Syntax => "syntax",
            serde_json::error::Category::Data => "data",
            serde_json::error::Category::Eof => "unexpected end of input",
        };
        let location = if error.line() == 0 {
            String::new()
        } else {
            format!(" at line {} column {}", error.line(), error.column())
        };
        Self::EncodeFailed {
            what: what.into(),
            cause: format!("{category} error{location}"),
        }
    }

    /// Builds `INPUT_INVALID` for a failed read or write.
    ///
    /// Only the [`io::ErrorKind`] is kept, never the OS message, which can
    /// echo paths or payload bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::JevError;
    ///
    /// let cause = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "secret");
    /// let error = JevError::io_failed("stdout", &cause);
    /// assert_eq!(error.code(), "INPUT_INVALID");
    /// assert!(error.fix_hint().contains("broken pipe"));
    /// assert!(!error.fix_hint().contains("secret"));
    /// ```
    #[must_use]
    pub fn io_failed(what: impl Into<String>, error: &io::Error) -> Self {
        Self::IoFailed {
            what: what.into(),
            kind: error.kind(),
        }
    }

    /// `KEY_MISSING` for `backend`.
    #[must_use]
    pub fn key_missing(backend: BackendKeyId) -> Self {
        Self::KeyMissing { backend }
    }

    /// `INPUT_TOO_LARGE`.
    #[must_use]
    pub fn input_too_large(limit_bytes: u64, actual_bytes: u64) -> Self {
        Self::InputTooLarge {
            limit_bytes,
            actual_bytes,
        }
    }

    /// `INPUT_INVALID` with the check-schema hint.
    #[must_use]
    pub fn input_invalid(detail: impl Into<String>) -> Self {
        Self::InputInvalid {
            detail: detail.into(),
        }
    }

    /// `EXTRACTION_PARSE_ERROR` at `path:line`.
    #[must_use]
    pub fn extraction_parse_error(path: impl Into<String>, line: u32) -> Self {
        Self::ExtractionParseError {
            path: path.into(),
            line,
        }
    }

    /// `EXTRACTION_INVALID`.
    #[must_use]
    pub fn extraction_invalid(detail: impl Into<String>) -> Self {
        Self::ExtractionInvalid {
            detail: detail.into(),
        }
    }

    /// `EXTRACTION_FAILED`.
    #[must_use]
    pub fn extraction_failed(detail: impl Into<String>) -> Self {
        Self::ExtractionFailed {
            detail: detail.into(),
        }
    }

    /// `EXTRACTION_TIMEOUT`.
    #[must_use]
    pub fn extraction_timeout(timeout_seconds: u64) -> Self {
        Self::ExtractionTimeout { timeout_seconds }
    }

    /// `LANGUAGE_UNAVAILABLE` from `producer`.
    #[must_use]
    pub fn language_unavailable(producer: impl Into<String>, language: impl Into<String>) -> Self {
        Self::LanguageUnavailable {
            producer: producer.into(),
            language: language.into(),
        }
    }

    /// `QUERY_INVALID` from `producer`.
    #[must_use]
    pub fn query_invalid(producer: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::QueryInvalid {
            producer: producer.into(),
            detail: detail.into(),
        }
    }

    /// `QUERY_NOT_FOUND` from `producer`.
    #[must_use]
    pub fn query_not_found(producer: impl Into<String>, query: impl Into<String>) -> Self {
        Self::QueryNotFound {
            producer: producer.into(),
            query: query.into(),
        }
    }

    /// A producer's stable error, re-raised; `code()` returns `code`.
    #[must_use]
    pub fn producer_error(
        producer: impl Into<String>,
        code: ErrorCode,
        message: impl Into<String>,
    ) -> Self {
        Self::ProducerError {
            producer: producer.into(),
            code,
            message: message.into(),
        }
    }

    /// `INPUT_MISSING` for `path`.
    #[must_use]
    pub fn input_missing(path: impl Into<String>) -> Self {
        Self::InputMissing { path: path.into() }
    }

    /// `RESPONSE_INVALID`.
    #[must_use]
    pub fn response_invalid(detail: impl Into<String>) -> Self {
        Self::ResponseInvalid {
            detail: detail.into(),
        }
    }

    /// `NETWORK_ERROR` of class `kind`. `detail` must already be redacted.
    #[must_use]
    pub fn network(kind: NetworkErrorKind, detail: impl Into<String>) -> Self {
        Self::NetworkError {
            kind,
            detail: detail.into(),
        }
    }

    /// `PROMPT_FILE_MISSING` for `path`.
    #[must_use]
    pub fn prompt_file_missing(path: impl Into<String>) -> Self {
        Self::PromptFileMissing { path: path.into() }
    }

    /// `SESSION_NOT_FOUND` for `session_id`.
    #[must_use]
    pub fn session_not_found(session_id: SessionId) -> Self {
        Self::SessionNotFound { session_id }
    }

    /// `SESSION_EXPIRED` for `session_id`.
    #[must_use]
    pub fn session_expired(session_id: SessionId) -> Self {
        Self::SessionExpired { session_id }
    }

    /// Re-raises a producer's stderr error response.
    ///
    /// A known id becomes [`JevError::ProducerError`]: its `code()` is that
    /// id and its fix hint is `"{producer}: {message}"`. An unknown id
    /// becomes `EXTRACTION_FAILED` naming the id.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::{JevError, ScAgentErrorResponse};
    ///
    /// let line = r#"{"error":"LANGUAGE_UNAVAILABLE","message":"Use python.","schema_version":1}"#;
    /// let response: ScAgentErrorResponse = serde_json::from_str(line).unwrap();
    /// let error = JevError::from_error_response("sc-treesitter", &response);
    /// assert_eq!(error.code(), "LANGUAGE_UNAVAILABLE");
    /// assert_eq!(error.fix_hint(), "sc-treesitter: Use python.");
    /// ```
    #[must_use]
    pub fn from_error_response(producer: &str, response: &ScAgentErrorResponse) -> Self {
        match response.error().parse::<ErrorCode>() {
            Ok(code) => Self::producer_error(producer, code, response.message()),
            Err(_) => Self::extraction_failed(format!(
                "{producer} reported unknown error id {}",
                response.error()
            )),
        }
    }

    /// Rewrites every text field of this error with `map`.
    ///
    /// Covers every `String` field of every variant. Variants without text
    /// are returned unchanged. The match is exhaustive, so adding a variant
    /// with a text field fails to compile until it is handled here.
    /// `ResolvedApiKey::redact_error` is built on this method.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::JevError;
    ///
    /// let error = JevError::input_missing("a/b");
    /// let mapped = error.map_detail(|text| text.to_uppercase());
    /// assert_eq!(mapped, JevError::input_missing("A/B"));
    /// ```
    #[must_use]
    pub fn map_detail(self, map: impl Fn(&str) -> String) -> Self {
        match self {
            Self::ConfigInvalid { detail } => Self::config_invalid(map(&detail)),
            Self::CredentialStoreInvalid {
                what,
                cause,
                recovery,
            } => Self::credential_store_invalid(map(&what), map(&cause), map(&recovery)),
            Self::RegistryInvalid { detail, recovery } => {
                Self::registry_invalid(map(&detail), map(&recovery))
            }
            Self::SessionStoreIo { op, kind, recovery } => {
                Self::session_store_io(map(&op), kind, map(&recovery))
            }
            Self::Operational { area, recovery } => Self::Operational {
                area,
                recovery: map(&recovery),
            },
            Self::SessionDocumentInvalid { detail, recovery } => {
                Self::session_document_invalid(map(&detail), map(&recovery))
            }
            Self::EncodeFailed { what, cause } => Self::EncodeFailed {
                what: map(&what),
                cause: map(&cause),
            },
            Self::IoFailed { what, kind } => Self::IoFailed {
                what: map(&what),
                kind,
            },
            Self::InputInvalid { detail } => Self::input_invalid(map(&detail)),
            Self::ExtractionParseError { path, line } => {
                Self::extraction_parse_error(map(&path), line)
            }
            Self::ExtractionInvalid { detail } => Self::extraction_invalid(map(&detail)),
            Self::ExtractionFailed { detail } => Self::extraction_failed(map(&detail)),
            Self::LanguageUnavailable { producer, language } => {
                Self::language_unavailable(map(&producer), map(&language))
            }
            Self::QueryInvalid { producer, detail } => {
                Self::query_invalid(map(&producer), map(&detail))
            }
            Self::QueryNotFound { producer, query } => {
                Self::query_not_found(map(&producer), map(&query))
            }
            Self::ProducerError {
                producer,
                code,
                message,
            } => Self::producer_error(map(&producer), code, map(&message)),
            Self::InputMissing { path } => Self::input_missing(map(&path)),
            Self::ResponseInvalid { detail } => Self::response_invalid(map(&detail)),
            Self::NetworkError { kind, detail } => Self::network(kind, map(&detail)),
            Self::PromptFileMissing { path } => Self::prompt_file_missing(map(&path)),
            unchanged @ (Self::KeyMissing { .. }
            | Self::InputTooLarge { .. }
            | Self::InputEmpty
            | Self::ExtractionEmpty
            | Self::ExtractionTimeout { .. }
            | Self::SessionNotFound { .. }
            | Self::SessionExpired { .. }) => unchanged,
        }
    }

    /// Returns the stable machine-readable id for this error.
    ///
    /// Ids match `tests/fixtures/jev/README-contract/errors.json` and are the
    /// only values CLI and PyO3 may emit in the `error` field. Several
    /// variants share an id (for example `RegistryInvalid` and
    /// `Operational { area: Config }` are both `CONFIG_INVALID`).
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::ConfigInvalid { .. }
            | Self::CredentialStoreInvalid { .. }
            | Self::RegistryInvalid { .. }
            | Self::SessionStoreIo { .. } => ErrorCode::ConfigInvalid,
            Self::Operational { area, .. } => area.code(),
            Self::SessionDocumentInvalid { .. }
            | Self::EncodeFailed { .. }
            | Self::IoFailed { .. }
            | Self::InputInvalid { .. } => ErrorCode::InputInvalid,
            Self::KeyMissing { .. } => ErrorCode::KeyMissing,
            Self::InputTooLarge { .. } => ErrorCode::InputTooLarge,
            Self::InputEmpty => ErrorCode::InputEmpty,
            Self::ExtractionEmpty => ErrorCode::ExtractionEmpty,
            Self::ExtractionParseError { .. } => ErrorCode::ExtractionParseError,
            Self::ExtractionInvalid { .. } => ErrorCode::ExtractionInvalid,
            Self::ExtractionFailed { .. } => ErrorCode::ExtractionFailed,
            Self::ExtractionTimeout { .. } => ErrorCode::ExtractionTimeout,
            Self::LanguageUnavailable { .. } => ErrorCode::LanguageUnavailable,
            Self::QueryInvalid { .. } => ErrorCode::QueryInvalid,
            Self::QueryNotFound { .. } => ErrorCode::QueryNotFound,
            Self::ProducerError { code, .. } => *code,
            Self::InputMissing { .. } => ErrorCode::InputMissing,
            Self::ResponseInvalid { .. } => ErrorCode::ResponseInvalid,
            Self::NetworkError { .. } => ErrorCode::NetworkError,
            Self::PromptFileMissing { .. } => ErrorCode::PromptFileMissing,
            Self::SessionNotFound { .. } => ErrorCode::SessionNotFound,
            Self::SessionExpired { .. } => ErrorCode::SessionExpired,
        }
    }

    /// Whether retrying the same request later may succeed.
    ///
    /// True only for `NetworkError` of kind `Busy`, `Timeout`, or `Connect`.
    #[must_use]
    pub fn retryable(&self) -> bool {
        matches!(self, Self::NetworkError { kind, .. } if kind.retryable())
    }

    /// Returns the actionable fix text for this error.
    ///
    /// The CLI copies this string into the stderr `message` field. It is the
    /// one renderer of recovery text: the `recovery` fields are data it
    /// formats. Plain `CONFIG_INVALID` keeps the `check.toml` hint and plain
    /// `INPUT_INVALID` keeps the schema hint; the explicit recovery variants
    /// do not mention either.
    #[must_use]
    pub fn fix_hint(&self) -> String {
        match self {
            Self::ConfigInvalid { detail } => format!(
                "Fix the check.toml field or packaged jev.toml entry ({detail}), then retry."
            ),
            Self::CredentialStoreInvalid {
                what,
                cause,
                recovery,
            } => format!("{what} (cause: {cause}). {recovery}"),
            Self::RegistryInvalid { detail, recovery }
            | Self::SessionDocumentInvalid { detail, recovery } => {
                format!("{detail}. {recovery}")
            }
            Self::SessionStoreIo { op, kind, recovery } => {
                format!("{op} failed (cause: {kind}). {recovery}")
            }
            Self::Operational { recovery, .. } => recovery.clone(),
            Self::EncodeFailed { what, cause } => format!(
                "{what} could not be encoded as JSON ({cause}). Retry; if it persists, report it as a bug."
            ),
            Self::IoFailed { what, kind } => format!(
                "{what} I/O failed (cause: {kind}). Check that it is readable or writable and retry."
            ),
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
            Self::LanguageUnavailable { producer, language } => {
                format!("Use a language built into {producer} instead of {language}.")
            }
            Self::QueryInvalid { detail, .. } => {
                format!("Fix the tree-sitter query syntax ({detail}) and retry.")
            }
            Self::QueryNotFound { query, .. } => {
                format!("Choose a query asset that exists for this language instead of {query}.")
            }
            Self::ProducerError {
                producer, message, ..
            } => format!("{producer}: {message}"),
            Self::InputMissing { path } => {
                format!("Create {path} or correct the path in check.toml, then retry.")
            }
            Self::ResponseInvalid { detail } => format!(
                "Retry the request. The judge response was invalid ({detail}). If it persists, treat a truncated or off-schema response as a backend failure."
            ),
            Self::NetworkError { detail, .. } => format!(
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

    /// Completeness of [`ErrorCode::ALL`]: this `match` has no wildcard arm,
    /// so a new variant fails to compile until it is listed here, and each
    /// arm asserts the variant is in `ALL`.
    #[test]
    fn test_error_code_all_is_complete() {
        let in_all = |code: ErrorCode| assert!(ErrorCode::ALL.contains(&code), "{code}");
        for code in ErrorCode::ALL.iter().copied() {
            match code {
                ErrorCode::ConfigInvalid => in_all(ErrorCode::ConfigInvalid),
                ErrorCode::KeyMissing => in_all(ErrorCode::KeyMissing),
                ErrorCode::InputTooLarge => in_all(ErrorCode::InputTooLarge),
                ErrorCode::InputEmpty => in_all(ErrorCode::InputEmpty),
                ErrorCode::InputInvalid => in_all(ErrorCode::InputInvalid),
                ErrorCode::ExtractionEmpty => in_all(ErrorCode::ExtractionEmpty),
                ErrorCode::ExtractionParseError => in_all(ErrorCode::ExtractionParseError),
                ErrorCode::ExtractionInvalid => in_all(ErrorCode::ExtractionInvalid),
                ErrorCode::ExtractionFailed => in_all(ErrorCode::ExtractionFailed),
                ErrorCode::ExtractionTimeout => in_all(ErrorCode::ExtractionTimeout),
                ErrorCode::LanguageUnavailable => in_all(ErrorCode::LanguageUnavailable),
                ErrorCode::QueryInvalid => in_all(ErrorCode::QueryInvalid),
                ErrorCode::QueryNotFound => in_all(ErrorCode::QueryNotFound),
                ErrorCode::InputMissing => in_all(ErrorCode::InputMissing),
                ErrorCode::ResponseInvalid => in_all(ErrorCode::ResponseInvalid),
                ErrorCode::NetworkError => in_all(ErrorCode::NetworkError),
                ErrorCode::PromptFileMissing => in_all(ErrorCode::PromptFileMissing),
                ErrorCode::SessionNotFound => in_all(ErrorCode::SessionNotFound),
                ErrorCode::SessionExpired => in_all(ErrorCode::SessionExpired),
            }
        }
        let unique: std::collections::BTreeSet<_> = ErrorCode::ALL.iter().collect();
        assert_eq!(unique.len(), ErrorCode::ALL.len());
    }

    /// Number of `String` fields in `error`, written without `map_detail`.
    fn text_fields(error: &JevError) -> usize {
        match error {
            JevError::CredentialStoreInvalid { .. } => 3,
            JevError::ProducerError { .. }
            | JevError::RegistryInvalid { .. }
            | JevError::SessionStoreIo { .. }
            | JevError::SessionDocumentInvalid { .. }
            | JevError::EncodeFailed { .. }
            | JevError::LanguageUnavailable { .. }
            | JevError::QueryInvalid { .. }
            | JevError::QueryNotFound { .. } => 2,
            JevError::ConfigInvalid { .. }
            | JevError::Operational { .. }
            | JevError::IoFailed { .. }
            | JevError::InputInvalid { .. }
            | JevError::ExtractionParseError { .. }
            | JevError::ExtractionInvalid { .. }
            | JevError::ExtractionFailed { .. }
            | JevError::InputMissing { .. }
            | JevError::ResponseInvalid { .. }
            | JevError::NetworkError { .. }
            | JevError::PromptFileMissing { .. } => 1,
            JevError::KeyMissing { .. }
            | JevError::InputTooLarge { .. }
            | JevError::InputEmpty
            | JevError::ExtractionEmpty
            | JevError::ExtractionTimeout { .. }
            | JevError::SessionNotFound { .. }
            | JevError::SessionExpired { .. } => 0,
        }
    }

    fn samples() -> Vec<JevError> {
        let session = SessionId::from_uuid(uuid::Uuid::nil());
        let cause = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
        vec![
            JevError::config_invalid("missing model"),
            JevError::credential_store_invalid("the .env file", "permission denied", "Fix it."),
            JevError::registry_invalid("Backend jev is not registered", "Register it."),
            JevError::session_store_io("Write s", io::ErrorKind::PermissionDenied, "Retry."),
            JevError::operational_config("Pass --repo-root."),
            JevError::operational_io("Retry stdout."),
            JevError::session_document_invalid("Session file s is bad", "Delete it."),
            JevError::encode_failed("payload", &cause),
            JevError::io_failed("stdout", &io::Error::from(io::ErrorKind::BrokenPipe)),
            JevError::key_missing(BackendKeyId::TypeSafe),
            JevError::input_too_large(10, 11),
            JevError::InputEmpty,
            JevError::input_invalid("bad json"),
            JevError::ExtractionEmpty,
            JevError::extraction_parse_error("a.py", 3),
            JevError::extraction_invalid("schema"),
            JevError::extraction_failed("exit 1"),
            JevError::extraction_timeout(30),
            JevError::language_unavailable("sc-treesitter", "go"),
            JevError::query_invalid("sc-treesitter", "syntax"),
            JevError::query_not_found("sc-treesitter", "missing"),
            JevError::producer_error("sc-treesitter", ErrorCode::QueryInvalid, "bad"),
            JevError::input_missing("gone.toml"),
            JevError::response_invalid("truncated"),
            JevError::network(NetworkErrorKind::Busy, "reset"),
            JevError::prompt_file_missing("prompt.md"),
            JevError::session_not_found(session),
            JevError::session_expired(session),
        ]
    }

    #[test]
    fn map_detail_touches_every_text_field_of_every_variant() {
        const MARK: &str = "<<mapped>>";
        for error in samples() {
            let calls = std::cell::Cell::new(0_usize);
            let mapped = error.clone().map_detail(|text| {
                calls.set(calls.get() + 1);
                format!("{MARK}{text}")
            });
            assert_eq!(calls.get(), text_fields(&error), "{error:?}");
            assert_eq!(mapped.code(), error.code());
            if text_fields(&error) == 0 {
                assert_eq!(mapped, error);
            } else {
                assert_ne!(mapped, error, "{error:?}");
                assert!(mapped.fix_hint().contains(MARK) || mapped.to_string().contains(MARK));
            }
        }
    }

    #[test]
    fn source_is_none_for_every_variant() {
        use std::error::Error as _;
        for error in samples() {
            assert!(error.source().is_none(), "{error:?}");
        }
    }

    #[test]
    fn explicit_recovery_variants_do_not_mention_check_toml_or_schema() {
        for error in samples() {
            let hint = error.fix_hint();
            assert!(!hint.trim().is_empty(), "{error:?}");
            if !matches!(
                error,
                JevError::ConfigInvalid { .. }
                    | JevError::InputInvalid { .. }
                    | JevError::InputMissing { .. }
                    | JevError::PromptFileMissing { .. }
            ) {
                assert!(!hint.contains("check.toml"), "{hint}");
                assert!(!hint.contains("expected schema"), "{hint}");
            }
        }
    }
}
