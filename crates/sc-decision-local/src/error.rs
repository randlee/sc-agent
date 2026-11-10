//! [`LocalError`]: construction and discovery errors of sc-decision-local, with `local.*` ids.

use std::error::Error;
use std::fmt;
use std::time::Duration;

use sc_agent_http::HttpError;
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
    /// [`LocalBackend::discover`](crate::LocalBackend::discover) has replaced every copy of
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
        /// The serde_json error, unchanged; returned by `source()` and `cause_detail()`.
        source: serde_json::Error,
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
            Self::ProbeParse { source, .. } => Some(ErrorDetail::foreign(
                format!("serde_json.{:?}", source.classify()).to_lowercase(),
                source,
            )),
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
            Self::ProbeParse { source, .. } => Some(source),
            Self::ProbeStatus { .. } | Self::ModelNotServed { .. } | Self::LoadTimeout { .. } => {
                None
            }
        }
    }
}
