//! [`DecisionError`]: the one error type of the decision layer, with stable `decision.*` ids.

use std::error::Error;
use std::fmt;

use sc_agent_http::HttpError;
use sc_agent_types::{ErrorDetail, ErrorId};

use crate::api_error::{HttpBadRequestError, HttpValidationError, LocItem};
use crate::map::duplicate_message;

/// Every error of a decision: parsing, encoding, the provider's refusal, the send, the
/// backend, cancellation (REQ-SAG-0002).
///
/// `Display` writes `"<id>: <message>"` only; the lower error stays in [`Error::source`] and
/// [`ErrorId::detail`]. Wrapping keeps the lower value unchanged, so every id and message
/// reaches the top.
///
/// # Examples
///
/// ```
/// use sc_decision_schema::{DecisionError, SystemOneRequest};
/// use sc_agent_types::ErrorId;
///
/// let error = SystemOneRequest::from_json(r#"{"model":"m","state":"s","questions":{"q":{"type":"score"}}}"#)
///     .unwrap_err();
/// assert_eq!(error.id(), "decision.request_parse");
/// assert!(matches!(&error, DecisionError::RequestParse { path, .. } if path == "questions.q"));
/// assert_eq!(error.to_string(), format!("decision.request_parse: {}", error.message()));
/// ```
#[derive(Debug)]
#[non_exhaustive]
pub enum DecisionError {
    /// Request text is not a `SystemOneRequest`.
    RequestParse {
        /// The serde_path_to_error field path (e.g. `questions.q1`, `.` at the root).
        path: String,
        /// The full serde_json message with line and column (e.g. "missing field `criteria`").
        message: String,
    },
    /// A body of HTTP `status` is not the expected `SystemOneResponse` or `ModelMetadataList`.
    ResponseParse {
        /// The reply's HTTP status.
        status: u16,
        /// The expected type name.
        expected: &'static str,
        /// The serde_json message.
        message: String,
    },
    /// `OrderedMap::try_from_pairs` got a key twice.
    DuplicateKey {
        /// The duplicate key.
        key: String,
    },
    /// Serialising a body or building multipart failed.
    Encode {
        /// What failed.
        message: String,
    },
    /// `DecisionRequest::new`: an `Asset.part` with no (or more than one) matching `ImagePart`,
    /// or an `ImagePart` no asset references.
    ImageParts {
        /// Names the asset id and part name.
        detail: String,
    },
    /// `DecisionRequest::new`: a REQ-SAG-0014 limit exceeded.
    Limits {
        /// Which limit.
        what: LimitKind,
        /// The limit.
        max: u64,
    },
    /// HTTP 400 with the documented body.
    ApiBadRequest {
        /// The body.
        body: HttpBadRequestError,
    },
    /// HTTP 422 with the documented body.
    ApiValidation {
        /// The body.
        body: HttpValidationError,
    },
    /// Any other non-2xx, or a 400/422 body of another shape.
    ApiStatus {
        /// The reply's HTTP status.
        status: u16,
        /// The reply body, unchanged.
        body: Vec<u8>,
    },
    /// The send failed in sc-agent-http (provider busy, quota exhausted, network, ...),
    /// wrapped unchanged.
    Http(HttpError),
    /// A backend crate above this one failed (e.g. an LLM provider error chain), carried whole.
    /// Its recovery hint is the carried `detail().cause.recovery`; `recovery()` is `None` here.
    Backend {
        /// The backend's name.
        backend: &'static str,
        /// The backend's error, with its whole chain.
        detail: ErrorDetail,
    },
    /// The reply does not match the required schema.
    ///
    /// The runner resends the request unchanged up to `max_sends`, then returns this, with
    /// what did not match in `detail` (REQ-SAG-0901).
    SchemaMismatch {
        /// The backend's name.
        backend: &'static str,
        /// What did not match.
        detail: ErrorDetail,
    },
    /// The request did not finish before its `CancelToken` was cancelled (queued, held off,
    /// or in flight).
    Cancelled {
        /// Sends made before the cancel.
        sends: u32,
    },
    /// The runner stopped before the request finished (a worker thread could not start or
    /// stopped).
    Shutdown {
        /// Why it stopped.
        reason: String,
    },
}

impl From<HttpError> for DecisionError {
    fn from(error: HttpError) -> Self {
        Self::Http(error)
    }
}

impl ErrorId for DecisionError {
    fn id(&self) -> &'static str {
        match self {
            Self::RequestParse { .. } => "decision.request_parse",
            Self::ResponseParse { .. } => "decision.response_parse",
            Self::DuplicateKey { .. } => "decision.duplicate_key",
            Self::Encode { .. } => "decision.encode",
            Self::ImageParts { .. } => "decision.image_parts",
            Self::Limits { .. } => "decision.limits",
            Self::ApiBadRequest { .. } => "decision.api_bad_request",
            Self::ApiValidation { .. } => "decision.api_validation",
            Self::ApiStatus { .. } => "decision.api_status",
            Self::Http(_) => "decision.http",
            Self::Backend { .. } => "decision.backend",
            Self::SchemaMismatch { .. } => "decision.schema_mismatch",
            Self::Cancelled { .. } => "decision.cancelled",
            Self::Shutdown { .. } => "decision.shutdown",
        }
    }

    fn message(&self) -> String {
        match self {
            Self::RequestParse { path, message } => {
                format!("the request is not a SystemOneRequest at `{path}`: {message}")
            }
            Self::ResponseParse {
                status,
                expected,
                message,
            } => format!("the HTTP {status} body is not a {expected}: {message}"),
            Self::DuplicateKey { key } => duplicate_message(key),
            Self::Encode { message } => message.clone(),
            Self::ImageParts { detail } => detail.clone(),
            Self::Limits { what, max } => {
                format!("the request exceeds the `{what}` limit of {max}")
            }
            Self::ApiBadRequest { body } => {
                let detail = match &body.detail {
                    serde_json::Value::String(text) => text.clone(),
                    other => other.to_string(),
                };
                format!("the API refused the request (HTTP 400): {detail}")
            }
            Self::ApiValidation { body } => {
                let fields = body
                    .detail
                    .iter()
                    .map(|error| format!("{}: {}", loc_path(&error.loc), error.msg))
                    .collect::<Vec<_>>()
                    .join("; ");
                format!("the API refused the request (HTTP 422): {fields}")
            }
            Self::ApiStatus { status, body } => format!(
                "the API answered HTTP {status}: {}",
                String::from_utf8_lossy(body)
            ),
            Self::Http(error) => format!("the send failed ({})", error.id()),
            Self::Backend { backend, detail } => {
                format!("the {backend} backend failed ({})", detail.id)
            }
            Self::SchemaMismatch { backend, .. } => {
                format!("the {backend} reply does not match the required schema")
            }
            Self::Cancelled { sends } => {
                format!("the request was cancelled after {sends} send(s)")
            }
            Self::Shutdown { reason } => {
                format!("the decision runner stopped before the request finished: {reason}")
            }
        }
    }

    fn cause_detail(&self) -> Option<ErrorDetail> {
        match self {
            Self::Http(error) => Some(error.detail()),
            Self::Backend { detail, .. } | Self::SchemaMismatch { detail, .. } => {
                Some(detail.clone())
            }
            _ => None,
        }
    }

    fn recovery(&self) -> Option<&'static str> {
        match self {
            Self::RequestParse { .. } => Some(
                "fix the field named in `path` (see the TypeSafe API docs for its shape) and send again",
            ),
            Self::DuplicateKey { .. } => Some("give every key of the object once"),
            Self::Encode { .. } => Some(
                "report this as a bug: a parsed request or reply always serialises; check values set in code",
            ),
            Self::ImageParts { .. } => Some(
                "give every image part a distinct name other than `request`, named by exactly one asset's `part`",
            ),
            Self::Limits { .. } => Some(
                "send at most 20 images per request, each at most 20 MiB; split larger work across requests",
            ),
            Self::ApiBadRequest { .. } => Some(
                "fix the request as the API's `detail` says (see the TypeSafe API docs) and send again",
            ),
            Self::ApiValidation { .. } => {
                Some("fix each field named in `loc` as its `msg` says and send again")
            }
            Self::SchemaMismatch { .. } => Some(
                "the provider kept replying outside the schema; try another model or backend, or raise max_sends",
            ),
            Self::Shutdown { .. } => Some("start a new decision runner and send the request again"),
            Self::Http(error) => error.recovery(),
            Self::ResponseParse { .. }
            | Self::ApiStatus { .. }
            | Self::Backend { .. }
            | Self::Cancelled { .. } => None,
        }
    }
}

/// Which REQ-SAG-0014 limit a [`DecisionError::Limits`] names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LimitKind {
    /// The number of images in one request ([`MAX_IMAGES`](crate::MAX_IMAGES)).
    Images,
    /// The bytes of one image ([`MAX_IMAGE_BYTES`](crate::MAX_IMAGE_BYTES)).
    ImageBytes,
}

impl LimitKind {
    /// The limit's name: `"images"` or `"image_bytes"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Images => "images",
            Self::ImageBytes => "image_bytes",
        }
    }
}

impl fmt::Display for LimitKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A 422 `loc` as `body.questions.q1`.
fn loc_path(loc: &[LocItem]) -> String {
    loc.iter()
        .map(|item| match item {
            LocItem::Name(name) => name.clone(),
            LocItem::Index(index) => index.to_string(),
        })
        .collect::<Vec<_>>()
        .join(".")
}

impl fmt::Display for DecisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.id(), self.message())
    }
}

impl Error for DecisionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Http(error) => Some(error),
            Self::Backend { detail, .. } | Self::SchemaMismatch { detail, .. } => Some(detail),
            _ => None,
        }
    }
}
