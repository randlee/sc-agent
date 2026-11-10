//! [`Generic`]: discovery for any Jev-compatible daemon (`GET /v1/models`).

use sc_agent_http::{HttpRequest, HttpResponse};
use sc_agent_secrets::ApiKey;
use sc_agent_types::ModelId;
use sc_decision_schema::ModelMetadataList;
use sc_decision_systemone::BaseUrl;

use crate::daemon::{
    Availability, DaemonKind, LocalDaemon, MODELS_PATH, ModelState, get, probe_status, sealed,
};
use crate::error::LocalError;

/// A generic Jev-compatible daemon: the only documented discovery endpoint is the TypeSafe
/// `GET /v1/models`.
///
/// A 2xx [`ModelMetadataList`] listing the configured model is [`ModelState::Loaded`]; one
/// that does not is [`LocalError::ModelNotServed`]. A 404 or 405 (no `/v1/models`) is
/// [`ModelState::Unlisted`]; any other status is [`LocalError::ProbeStatus`].
#[derive(Debug, Clone, Copy, Default)]
pub struct Generic;

impl sealed::Sealed for Generic {}

impl LocalDaemon for Generic {
    fn kind(&self) -> DaemonKind {
        DaemonKind::Generic
    }

    /// `GET {base}/v1/models`, with the Bearer key when one is configured.
    fn probe(&self, base: &BaseUrl, key: Option<&ApiKey>) -> Result<HttpRequest, LocalError> {
        get(base, MODELS_PATH, key)
    }

    fn availability(
        &self,
        response: &HttpResponse,
        model: &ModelId,
    ) -> Result<Availability, LocalError> {
        let daemon = self.kind();
        let status = response.status();
        match status {
            200..=299 => {
                let list = ModelMetadataList::from_json(status, response.body())?;
                let loaded: Vec<String> = list.models.into_iter().map(|entry| entry.name).collect();
                if loaded.iter().any(|name| name == model.as_str()) {
                    Ok(Availability {
                        daemon,
                        loaded,
                        model: ModelState::Loaded,
                    })
                } else {
                    Err(LocalError::ModelNotServed {
                        daemon,
                        model: model.clone(),
                        served: loaded,
                    })
                }
            }
            404 | 405 => Ok(Availability {
                daemon,
                loaded: Vec::new(),
                model: ModelState::Unlisted,
            }),
            _ => Err(probe_status(daemon, MODELS_PATH, response)),
        }
    }
}
