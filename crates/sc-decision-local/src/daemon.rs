//! [`LocalDaemon`]: one daemon kind's sans-IO discovery, and the values it reports.

use std::fmt;

use sc_agent_config::RunnerKind;
use sc_agent_http::{Endpoint, HttpRequest, HttpResponse, Method, SecretHeader};
use sc_agent_secrets::ApiKey;
use sc_agent_types::ModelId;
use serde::de::DeserializeOwned;

use crate::error::LocalError;
use crate::{Generic, Laya, Mapika, Strands};

pub(crate) mod sealed {
    /// Private supertrait of [`LocalDaemon`](super::LocalDaemon): only this crate implements it.
    pub trait Sealed {}
}

/// One daemon kind's discovery, sans-IO: build the probe, read its reply.
///
/// Sealed: the implementors are [`Strands`], [`Laya`], [`Mapika`] and [`Generic`]. A new
/// daemon is added inside this crate as a [`DaemonKind`] variant ([`DaemonKind::daemon`] is
/// the extension path).
pub trait LocalDaemon: sealed::Sealed + Send + Sync + fmt::Debug {
    /// The daemon kind.
    fn kind(&self) -> DaemonKind;

    /// The one GET that tells whether the daemon at `base` is up and what it has loaded.
    ///
    /// `key` is attached as Bearer only by kinds that check it (Laya, generic).
    ///
    /// # Errors
    ///
    /// [`LocalError::Http`] when the probe URL is invalid, or when a key is attached to a
    /// plain-http host that is not loopback (`http.key_on_plain_http`).
    fn probe(&self, base: &str, key: Option<&ApiKey>) -> Result<HttpRequest, LocalError>;

    /// What the probe reply says about the daemon and the configured `model`.
    ///
    /// # Errors
    ///
    /// [`LocalError::ProbeStatus`] for a status the kind does not answer when up,
    /// [`LocalError::ProbeParse`] (or [`LocalError::Decision`] for a model list) for a body
    /// of another shape, [`LocalError::ModelNotServed`] when the daemon is up but cannot
    /// serve `model`.
    fn availability(
        &self,
        response: &HttpResponse,
        model: &ModelId,
    ) -> Result<Availability, LocalError>;
}

/// The daemon kinds sc-decision-local can discover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DaemonKind {
    /// Strands Decider (`GET /health`, no auth).
    Strands,
    /// Laya (`GET /health`, Bearer when a key is configured; loads checkpoints on demand).
    Laya,
    /// Mapika/decider (`GET /health`, no auth).
    Mapika,
    /// Any other Jev-compatible daemon (`GET /v1/models`, Bearer when a key is configured).
    Generic,
}

/// Total: [`RunnerKind`] is a closed enum checked at config load (REQ-SAG-0013), so a new
/// runner kind is a compile error here, not a runtime error.
impl From<RunnerKind> for DaemonKind {
    fn from(kind: RunnerKind) -> Self {
        match kind {
            RunnerKind::Strands => Self::Strands,
            RunnerKind::Laya => Self::Laya,
            RunnerKind::Mapika => Self::Mapika,
            RunnerKind::Generic => Self::Generic,
        }
    }
}

impl DaemonKind {
    /// This kind's discovery implementation.
    #[must_use]
    pub fn daemon(self) -> Box<dyn LocalDaemon> {
        match self {
            Self::Strands => Box::new(Strands),
            Self::Laya => Box::new(Laya),
            Self::Mapika => Box::new(Mapika),
            Self::Generic => Box::new(Generic),
        }
    }

    /// The kind's name as a log field: `strands`, `laya`, `mapika` or `generic`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Strands => "strands",
            Self::Laya => "laya",
            Self::Mapika => "mapika",
            Self::Generic => "generic",
        }
    }

    /// Whether the daemon loads a model on its first request rather than at start (Laya).
    pub(crate) const fn loads_on_first_request(self) -> bool {
        matches!(self, Self::Laya)
    }
}

impl fmt::Display for DaemonKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The daemon is up: the probe got an HTTP answer it understood.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Availability {
    /// The daemon kind probed.
    pub daemon: DaemonKind,
    /// The models the daemon reports loaded or served, in its order.
    pub loaded: Vec<String>,
    /// Where the configured model stands.
    pub model: ModelState,
}

/// Where the configured model stands on an up daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModelState {
    /// The daemon reports the model loaded.
    Loaded,
    /// Not loaded yet; the daemon loads it on the first request (Laya).
    LoadsOnDemand,
    /// The daemon is up but still loading (Mapika/decider `ok: false`).
    NotReady,
    /// The daemon does not say (Laya without the key, a generic daemon without
    /// `/v1/models`).
    Unlisted,
}

impl ModelState {
    /// The state's name as a log field: `loaded`, `loads_on_demand`, `not_ready`, `unlisted`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Loaded => "loaded",
            Self::LoadsOnDemand => "loads_on_demand",
            Self::NotReady => "not_ready",
            Self::Unlisted => "unlisted",
        }
    }
}

/// `GET {base}{path}`, with the Bearer key when `key` is set.
pub(crate) fn get(base: &str, path: &str, key: Option<&ApiKey>) -> Result<HttpRequest, LocalError> {
    let base = base.strip_suffix('/').unwrap_or(base);
    let endpoint = Endpoint::parse_local(&format!("{base}{path}"))?;
    let request = HttpRequest::new(Method::Get, endpoint, Vec::new());
    Ok(match key {
        Some(key) => request.with_secret(SecretHeader::bearer(key.clone()))?,
        None => request,
    })
}

/// The reply body of a probe as `T`.
pub(crate) fn parse<T: DeserializeOwned>(
    daemon: DaemonKind,
    path: &'static str,
    response: &HttpResponse,
) -> Result<T, LocalError> {
    serde_json::from_slice(response.body()).map_err(|error| LocalError::ProbeParse {
        daemon,
        path,
        message: error.to_string(),
    })
}

/// [`LocalError::ProbeStatus`] for `response`.
pub(crate) fn probe_status(
    daemon: DaemonKind,
    path: &'static str,
    response: &HttpResponse,
) -> LocalError {
    LocalError::ProbeStatus {
        daemon,
        path,
        status: response.status(),
        body: response.body().to_vec(),
    }
}

/// The liveness path of Strands Decider, Laya and Mapika/decider.
pub(crate) const HEALTH_PATH: &str = "/health";

/// The model-list path of a generic daemon.
pub(crate) const MODELS_PATH: &str = "/v1/models";
