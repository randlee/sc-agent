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
    InvalidVarName,
    /// Not a canonical ULID, a UUID included (`types.invalid_session_id`).
    InvalidSessionId {
        /// The rejected text.
        text: String,
    },
}

crate::error_catalog! { TypesError {
    TypesError::InvalidModelId { .. } => ("types.invalid_model_id", Some(crate::ErrorCategory::Validation)),
    TypesError::UnknownShortName { .. } => ("types.unknown_short_name", Some(crate::ErrorCategory::Validation)),
    TypesError::UnknownProvider { .. } => ("types.unknown_provider", Some(crate::ErrorCategory::Validation)),
    TypesError::InvalidReleaseDate { .. } => ("types.invalid_release_date", Some(crate::ErrorCategory::Config)),
    TypesError::ModelOnTwoLines { .. } => ("types.model_on_two_lines", Some(crate::ErrorCategory::Config)),
    TypesError::InvalidVarName => ("types.invalid_var_name", Some(crate::ErrorCategory::Validation)),
    TypesError::InvalidSessionId { .. } => ("types.invalid_session_id", Some(crate::ErrorCategory::Validation)),
} }

impl ErrorId for TypesError {
    fn id(&self) -> &'static str {
        self.catalog_row().0
    }
    fn base_category(&self) -> Option<crate::ErrorCategory> {
        self.catalog_row().1
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
            Self::InvalidVarName => "invalid environment-variable name (uppercase letters, digits and _, not starting with a digit, at most 128 characters)".to_owned(),
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
                "use a short name from the sc-agent-types ShortName registry"
            }
            Self::UnknownProvider { .. } => {
                "use a provider from the sc-agent-types Provider registry"
            }
            Self::InvalidReleaseDate { .. } => {
                "write the date as YYYY-MM-DD, for example 2026-10-14"
            }
            Self::ModelOnTwoLines { .. } => {
                "keep the model under its existing line or choose a different id"
            }
            Self::InvalidVarName => {
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
