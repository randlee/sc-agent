//! [`LocalError`]: construction and discovery errors of sc-decision-local, with `local.*` ids.

use std::borrow::Cow;
use std::error::Error;
use std::fmt;
use std::time::Duration;

use sc_agent_http::HttpError;
use sc_agent_secrets::ApiKey;
use sc_agent_types::{ErrorDetail, ErrorId, ModelId};
use sc_decision_schema::DecisionError;

use crate::daemon::DaemonKind;

const MODEL_NOT_SERVED_RECOVERY: &str =
    "load the configured model on the daemon, or set the runner's model to one it serves";
const PROBE_RECOVERY: &str = "check that the runner's kind matches the daemon at its host and port";
const LOAD_TIMEOUT_RECOVERY: &str =
    "raise the runner's load timeout or check that the daemon is loading the model";

/// Every error of building a [`LocalBackend`](crate::LocalBackend) or discovering its daemon.
///
/// `Display` writes `"<id>: <message>"` only; a wrapped [`HttpError`] or [`DecisionError`] is
/// kept unchanged and reachable through [`Error::source`] and [`ErrorId::detail`]
/// (REQ-SAG-0002). As a [`Backend`](sc_decision_schema::Backend), `LocalBackend` returns
/// `DecisionError`; this type is only for construction and discovery.
///
/// # Examples
///
/// ```
/// use sc_agent_http::{HttpError, NetworkKind};
/// use sc_agent_types::ErrorId;
/// use sc_decision_local::LocalError;
///
/// let error = LocalError::Http(HttpError::network(NetworkKind::Connect, 1, "connection refused"));
/// assert_eq!(error.id(), "local.http");
/// assert_eq!(error.detail().cause.map(|cause| cause.id), Some("http.network".to_owned()));
/// ```
#[derive(Debug)]
#[non_exhaustive]
pub enum LocalError {
    /// The probe could not be built or sent: daemon down (`http.network`) or bad endpoint;
    /// wrapped unchanged.
    Http(HttpError),
    /// `SystemOneBackend::local` or the model-list decode failed; wrapped unchanged.
    Decision(DecisionError),
    /// The probe got an HTTP status the daemon kind does not answer when up.
    ///
    /// The body is kept whole (REQ-SAG-0002); when the probe carried the runner's key,
    /// [`LocalBackend::read_probe`](crate::LocalBackend::read_probe) has replaced every copy of
    /// it in the body with `[redacted]`.
    ProbeStatus {
        /// The daemon kind probed.
        daemon: DaemonKind,
        /// The probed path, e.g. `/health`.
        path: &'static str,
        /// The reply's HTTP status.
        status: u16,
        /// The reply body, whole, with the attached key redacted.
        body: Vec<u8>,
    },
    /// The probe reply body is not the daemon kind's documented shape.
    ProbeParse {
        /// The daemon kind probed.
        daemon: DaemonKind,
        /// The probed path.
        path: &'static str,
        /// Why the body did not parse; returned by `source()` and `cause_detail()`.
        source: ParseCause,
    },
    /// The daemon is up but does not serve the configured model.
    ModelNotServed {
        /// The daemon kind probed.
        daemon: DaemonKind,
        /// The configured model.
        model: ModelId,
        /// What the daemon reports it serves.
        served: Vec<String>,
    },
    /// The model was not loaded within the load timeout (REQ-SAG-0015).
    LoadTimeout {
        /// The runner's base, `http://{host}:{port}`.
        runner: String,
        /// How long the wait ran.
        waited: Duration,
    },
}

impl ErrorId for LocalError {
    fn id(&self) -> &'static str {
        match self {
            Self::Http(_) => "local.http",
            Self::Decision(_) => "local.decision",
            Self::ProbeStatus { .. } => "local.probe_status",
            Self::ProbeParse { .. } => "local.probe_parse",
            Self::ModelNotServed { .. } => "local.model_not_served",
            Self::LoadTimeout { .. } => "local.load_timeout",
        }
    }

    fn message(&self) -> String {
        match self {
            Self::Http(error) => format!("the daemon probe failed ({})", error.id()),
            Self::Decision(error) => format!("the System One backend failed ({})", error.id()),
            Self::ProbeStatus {
                daemon,
                path,
                status,
                body,
            } => format!(
                "the {daemon} daemon answered GET {path} with HTTP {status}: {}",
                String::from_utf8_lossy(body)
            ),
            Self::ProbeParse { daemon, path, .. } => {
                format!("the {daemon} daemon's GET {path} body is not the shape that kind answers")
            }
            Self::ModelNotServed {
                daemon,
                model,
                served,
            } => format!(
                "the {daemon} daemon does not serve model {:?}; it serves {served:?}",
                model.as_str()
            ),
            Self::LoadTimeout { runner, waited } => format!(
                "the model at {runner} was not loaded after {} ms",
                waited.as_millis()
            ),
        }
    }

    fn cause_detail(&self) -> Option<ErrorDetail> {
        match self {
            Self::Http(error) => Some(error.detail()),
            Self::Decision(error) => Some(error.detail()),
            Self::ProbeParse { source, .. } => Some(source.detail()),
            Self::ProbeStatus { .. } | Self::ModelNotServed { .. } | Self::LoadTimeout { .. } => {
                None
            }
        }
    }

    fn recovery(&self) -> Option<&'static str> {
        match self {
            Self::ModelNotServed { .. } => Some(MODEL_NOT_SERVED_RECOVERY),
            Self::LoadTimeout { .. } => Some(LOAD_TIMEOUT_RECOVERY),
            Self::ProbeStatus { .. } | Self::ProbeParse { .. } => Some(PROBE_RECOVERY),
            Self::Http(_) | Self::Decision(_) => None,
        }
    }
}

/// The cause of a [`LocalError::ProbeParse`].
///
/// A serde_json message can quote a value from the reply body. When the probe carried the
/// runner's key, the cause is stored as [`ParseCause::Redacted`]: the same id
/// (`serde_json.<category>`) and message with every copy of the key replaced by
/// `[redacted]` (REQ-SAG-0002 0.2.0, the attached-key exception). Otherwise the serde_json error is kept unchanged.
#[derive(Debug)]
#[non_exhaustive]
pub enum ParseCause {
    /// The serde_json error, unchanged (no key was attached to the probe).
    Json(serde_json::Error),
    /// The serde_json error as data, with the attached key redacted from its message.
    Redacted(ErrorDetail),
}

impl ParseCause {
    /// The cause as data: id `serde_json.<category>` (`syntax`, `data`, `eof`, `io`) and the
    /// message.
    #[must_use]
    pub fn detail(&self) -> ErrorDetail {
        match self {
            Self::Json(error) => ErrorDetail::foreign(json_error_id(error), error),
            Self::Redacted(detail) => detail.clone(),
        }
    }

    fn as_error(&self) -> &(dyn Error + 'static) {
        match self {
            Self::Json(error) => error,
            Self::Redacted(detail) => detail,
        }
    }
}

/// `serde_json.<category>`, e.g. `serde_json.syntax`.
fn json_error_id(error: &serde_json::Error) -> String {
    format!("serde_json.{:?}", error.classify()).to_lowercase()
}

impl LocalError {
    /// This error with every copy of `key` in its text replaced by `[redacted]`, for an error
    /// read from a reply to a probe that carried `key`.
    ///
    /// Covers the ProbeStatus body, the ProbeParse cause, a model-list parse message and the
    /// served model names; the other variants hold no reply text.
    pub(crate) fn redact(self, key: &ApiKey) -> Self {
        match self {
            Self::ProbeStatus {
                daemon,
                path,
                status,
                body,
            } => Self::ProbeStatus {
                daemon,
                path,
                status,
                body: redact_bytes(key, body),
            },
            Self::ProbeParse {
                daemon,
                path,
                source,
            } => {
                let detail = source.detail();
                let id = detail.id;
                let message = key.redact_text(&detail.message).into_owned();
                Self::ProbeParse {
                    daemon,
                    path,
                    source: ParseCause::Redacted(ErrorDetail::new(id, message, None)),
                }
            }
            Self::Decision(DecisionError::ResponseParse {
                status,
                expected,
                message,
            }) => Self::Decision(DecisionError::ResponseParse {
                status,
                expected,
                message: redact_string(key, message),
            }),
            Self::ModelNotServed {
                daemon,
                model,
                served,
            } => Self::ModelNotServed {
                daemon,
                model,
                served: served
                    .into_iter()
                    .map(|name| redact_string(key, name))
                    .collect(),
            },
            other => other,
        }
    }
}

/// `text` with every copy of `key` replaced by `[redacted]`.
pub(crate) fn redact_string(key: &ApiKey, text: String) -> String {
    match key.redact_text(&text) {
        Cow::Owned(redacted) => redacted,
        Cow::Borrowed(_) => text,
    }
}

/// `body` with every copy of `key` replaced by `[redacted]`; every other byte unchanged
/// (REQ-SAG-0002).
///
/// Works on the raw bytes, so a body that is not UTF-8 keeps its invalid bytes. A key is
/// visible ASCII (sc-agent-secrets) and an ASCII byte is never part of an invalid UTF-8
/// sequence, so every copy of the key lies inside one valid UTF-8 chunk: redacting each
/// valid chunk and copying the invalid bytes as they are is exact.
fn redact_bytes(key: &ApiKey, body: Vec<u8>) -> Vec<u8> {
    let mut out: Option<Vec<u8>> = None;
    let mut written = 0;
    let mut offset = 0;
    for chunk in body.utf8_chunks() {
        let valid = chunk.valid();
        if let Cow::Owned(redacted) = key.redact_text(valid) {
            let out = out.get_or_insert_with(|| Vec::with_capacity(body.len()));
            out.extend_from_slice(body.get(written..offset).unwrap_or_default());
            out.extend_from_slice(redacted.as_bytes());
            written = offset + valid.len();
        }
        offset += valid.len() + chunk.invalid().len();
    }
    match out {
        Some(mut out) => {
            out.extend_from_slice(body.get(written..).unwrap_or_default());
            out
        }
        None => body,
    }
}

impl From<HttpError> for LocalError {
    fn from(error: HttpError) -> Self {
        Self::Http(error)
    }
}

impl From<DecisionError> for LocalError {
    fn from(error: DecisionError) -> Self {
        Self::Decision(error)
    }
}

impl fmt::Display for LocalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.id(), self.message())
    }
}

impl Error for LocalError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Http(error) => Some(error),
            Self::Decision(error) => Some(error),
            Self::ProbeParse { source, .. } => Some(source.as_error()),
            Self::ProbeStatus { .. } | Self::ModelNotServed { .. } | Self::LoadTimeout { .. } => {
                None
            }
        }
    }
}
