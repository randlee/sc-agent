//! The one error convention of the workspace: [`ErrorId`] and [`ErrorDetail`] (REQ-SAG-0002).

use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

/// The stable id, message, cause and recovery hint every sc-agent error enum provides.
///
/// `id()` is stable per variant and spelled `"<crate-prefix>.<variant_snake>"`, for example
/// `"http.provider_busy"` or `"types.invalid_model_id"`.
///
/// The trait is open to workspace crates only: every sc-agent error enum implements it, and it
/// is not an extension point for outside crates. [`Self::recovery`] and [`Self::detail`] are
/// provided.
///
/// Implementors must not override [`Self::detail`]; workspace review rejects an override. It
/// is the one place that assembles id, message, recovery, optional HTTP status and cause, which
/// keeps every printed error, session row and Python exception consistent with the accessors.
/// Rust cannot seal a provided method, so each crate's tests check that `detail()` equals the
/// value built from those accessors.
///
/// The cause method is `cause_detail`, not `cause`: the deprecated `std::error::Error::cause`
/// on the supertrait has that name, and `x.cause()` would be ambiguous (E0034) wherever
/// [`Error`] is in scope.
///
/// Rules for every implementing error type:
///
/// - `Display` writes `"<id>: <message>"` only, never the cause, so a reporter that walks
///   [`Error::source`] prints each level once;
/// - `source()` returns the wrapped lower error, and wrapping keeps that value unchanged;
/// - `detail()` carries the whole chain.
///
/// # Examples
///
/// ```
/// use sc_agent_types::{ErrorId, ModelId};
///
/// let error = "".parse::<ModelId>().unwrap_err();
/// assert_eq!(error.id(), "types.invalid_model_id");
/// assert_eq!(error.to_string(), format!("{}: {}", error.id(), error.message()));
/// assert_eq!(error.detail().id, "types.invalid_model_id");
/// ```
pub trait ErrorId: Error {
    /// The stable id of this error, `"<crate-prefix>.<variant_snake>"`.
    fn id(&self) -> &'static str;

    /// This layer's own message, without the cause.
    fn message(&self) -> String;

    /// The lower error that caused this one, whole: its id, message, recovery and cause chain.
    fn cause_detail(&self) -> Option<ErrorDetail>;

    /// What the caller does next, if this layer knows. The provided body returns `None`.
    fn recovery(&self) -> Option<&'static str> {
        None
    }

    /// The HTTP status for an error that represents an HTTP response, when available.
    fn status(&self) -> Option<u16> {
        None
    }

    /// This error as data: id, message, recovery, HTTP status and the cause chain.
    ///
    /// Must not be overridden (see the trait docs); workspace review rejects an override.
    fn detail(&self) -> ErrorDetail {
        ErrorDetail {
            id: self.id().to_owned(),
            message: self.message(),
            recovery: self.recovery().map(Box::<str>::from),
            cause: self.cause_detail().map(Box::new),
            status: self.status(),
        }
    }
}

/// An error as data, with its cause chain.
///
/// It carries an error from a crate that is not below the current one (an `LlmError` inside a
/// `DecisionError`), and it is what the CLI, PyO3 and session rows print. `Display` writes
/// `"<id>: <message>"` only; the chain is reachable through [`Error::source`] and the
/// [`cause`](Self::cause) field.
///
/// # Examples
///
/// ```
/// use sc_agent_types::ErrorDetail;
///
/// let lower = ErrorDetail::new("http.provider_busy", "the provider is busy", None);
/// let outer = ErrorDetail::new("decision.backend_failed", "the backend failed", Some(lower))
///     .with_recovery("retry later");
/// assert_eq!(outer.to_string(), "decision.backend_failed: the backend failed");
/// assert_eq!(outer.cause.as_ref().map(|cause| cause.id.as_str()), Some("http.provider_busy"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorDetail {
    /// The stable id, `"<crate-prefix>.<variant_snake>"` (or a foreign code, see [`Self::foreign`]).
    pub id: String,
    /// The message of this level, without the cause.
    pub message: String,
    /// What the caller does next, if known. Omitted from serde output when `None`.
    ///
    /// A boxed string keeps the deserialized representation compact while allowing details
    /// from session rows and PyO3 to own their recovery text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery: Option<Box<str>>,
    /// The lower error that caused this one. Omitted from serde output when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<Box<ErrorDetail>>,
    /// The HTTP status represented by this error, when available. Omitted when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
}

/// The id given to each level of a foreign error's `source()` chain below the top one, which
/// carries no code of its own.
const FOREIGN_SOURCE_ID: &str = "foreign.source";

impl ErrorDetail {
    /// An error detail with no recovery hint.
    #[must_use]
    pub fn new(id: impl Into<String>, message: impl Into<String>, cause: Option<Self>) -> Self {
        Self {
            id: id.into(),
            message: message.into(),
            recovery: None,
            cause: cause.map(Box::new),
            status: None,
        }
    }

    /// This detail with `recovery` as its recovery hint.
    #[must_use]
    pub fn with_recovery(self, recovery: &'static str) -> Self {
        Self {
            recovery: Some(Box::<str>::from(recovery)),
            ..self
        }
    }

    /// This detail with the numeric HTTP status represented by it.
    #[must_use]
    pub fn with_status(self, status: u16) -> Self {
        Self {
            status: Some(status),
            ..self
        }
    }

    /// A foreign error (io, serde_json, sc-config, sc-observability, ureq, sc-sha) as data.
    ///
    /// `id` is the foreign code or kind and `message` is the error's `Display`. Each level of
    /// its [`Error::source`] chain becomes a [`cause`](Self::cause) with id `"foreign.source"`
    /// and that level's `Display` as its message.
    #[must_use]
    pub fn foreign(id: impl Into<String>, error: &(dyn Error + 'static)) -> Self {
        Self {
            id: id.into(),
            message: error.to_string(),
            recovery: None,
            cause: error
                .source()
                .map(|source| Box::new(Self::foreign(FOREIGN_SOURCE_ID, source))),
            status: None,
        }
    }
}

impl fmt::Display for ErrorDetail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.id, self.message)
    }
}

impl Error for ErrorDetail {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause
            .as_deref()
            .map(|cause| cause as &(dyn Error + 'static))
    }
}
