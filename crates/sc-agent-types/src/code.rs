//! Stable error ids and the closed enums `JevError` variants carry.
//!
//! Private module, reached through the crate-root re-exports. Kept apart
//! from `error` because sc-boundary reports a type cycle inside one module.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::JevError;

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
