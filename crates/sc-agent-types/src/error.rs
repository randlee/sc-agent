//! [`TypesError`], the error enum of this crate.

use std::error::Error;
use std::fmt;

use crate::{ErrorDetail, ErrorId, ModelId, ShortName};

/// A value given to this crate did not parse, or a catalog row conflicts.
///
/// This is the bottom crate: nothing is wrapped, so [`ErrorId::cause_detail`] is always `None`.
/// Every variant has a static [`ErrorId::recovery`] hint saying what to write instead.
/// `Display` is `"<id>: <message>"`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TypesError {
    /// Model id empty or with a control character (`types.invalid_model_id`).
    InvalidModelId {
        /// The rejected text.
        text: String,
    },
    /// Not one of the nine short names (`types.unknown_short_name`).
    UnknownShortName {
        /// The rejected text.
        text: String,
    },
    /// Not one of the six providers (`types.unknown_provider`).
    UnknownProvider {
        /// The rejected text.
        text: String,
    },
    /// Not `YYYY-MM-DD` with month 1..=12 and day 1..=31 (`types.invalid_release_date`).
    InvalidReleaseDate {
        /// The rejected text.
        text: String,
    },
    /// A config model id already belongs to another line (`types.model_on_two_lines`).
    ModelOnTwoLines {
        /// The model id.
        model: ModelId,
        /// The line it was offered under.
        line: ShortName,
        /// The line it already belongs to.
        other_line: ShortName,
    },
    /// Not an environment-variable name, `^[A-Z_][A-Z0-9_]{0,127}$` (`types.invalid_var_name`).
    InvalidVarName {
        /// The rejected text.
        text: String,
    },
    /// Not a canonical ULID, a UUID included (`types.invalid_session_id`).
    InvalidSessionId {
        /// The rejected text.
        text: String,
    },
}

impl ErrorId for TypesError {
    fn id(&self) -> &'static str {
        match self {
            Self::InvalidModelId { .. } => "types.invalid_model_id",
            Self::UnknownShortName { .. } => "types.unknown_short_name",
            Self::UnknownProvider { .. } => "types.unknown_provider",
            Self::InvalidReleaseDate { .. } => "types.invalid_release_date",
            Self::ModelOnTwoLines { .. } => "types.model_on_two_lines",
            Self::InvalidVarName { .. } => "types.invalid_var_name",
            Self::InvalidSessionId { .. } => "types.invalid_session_id",
        }
    }

    fn message(&self) -> String {
        match self {
            Self::InvalidModelId { text } => {
                format!("model id {text:?} must be non-empty with no control characters")
            }
            Self::UnknownShortName { text } => format!("{text:?} is not a short name"),
            Self::UnknownProvider { text } => format!("{text:?} is not a provider"),
            Self::InvalidReleaseDate { text } => {
                format!("release date {text:?} is not YYYY-MM-DD with month 01..12 and day 01..31")
            }
            Self::ModelOnTwoLines {
                model,
                line,
                other_line,
            } => format!(
                "model {model} already belongs to line {other_line}; it cannot also be {line}"
            ),
            Self::InvalidVarName { text } => format!(
                "{text:?} is not an environment-variable name (uppercase letters, digits and _, not starting with a digit, at most 128 characters)"
            ),
            Self::InvalidSessionId { text } => format!(
                "session id {text:?} is not a ULID (26 uppercase Crockford base32 characters)"
            ),
        }
    }

    fn cause_detail(&self) -> Option<ErrorDetail> {
        None
    }

    fn recovery(&self) -> Option<&'static str> {
        Some(match self {
            Self::InvalidModelId { .. } => {
                "use a non-empty model id with no control characters, or a short name such as haiku"
            }
            Self::UnknownShortName { .. } => {
                "use one of: jev, haiku, sonnet, opus, fable, luna, terra, sol, astra"
            }
            Self::UnknownProvider { .. } => {
                "use one of: typesafe, anthropic, openai, openrouter, nous, together"
            }
            Self::InvalidReleaseDate { .. } => {
                "write the date as YYYY-MM-DD, for example 2026-10-14"
            }
            Self::ModelOnTwoLines { .. } => {
                "keep the model under its existing line or choose a different id"
            }
            Self::InvalidVarName { .. } => {
                "use a name matching ^[A-Z_][A-Z0-9_]{0,127}$, for example MY_RUNNER_TOKEN"
            }
            Self::InvalidSessionId { .. } => {
                "use a 26-character uppercase ULID, for example 01ARZ3NDEKTSV4RRFFQ69G5FAV"
            }
        })
    }
}

impl fmt::Display for TypesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.id(), self.message())
    }
}

impl Error for TypesError {}
