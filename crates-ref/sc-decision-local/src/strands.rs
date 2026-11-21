//! [`Strands`]: discovery for Strands Decider (`GET /health`).

use sc_agent_http::{HttpRequest, HttpResponse};
use sc_agent_secrets::ApiKey;
use sc_agent_types::ModelId;
use sc_decision_systemone::BaseUrl;
use serde::Deserialize;

use crate::daemon::{
    Availability, DaemonKind, HEALTH_PATH, LocalDaemon, ModelState, get, parse, probe_status,
    sealed,
};
use crate::error::LocalError;

/// Strands Decider: one model per process, no auth, binds only after the model loads.
///
/// `GET /health` answers 200 `{"status":"ok","model":<served name>,"checkpoint":<path or
/// HF id>,...}`. The configured model is [`ModelState::Loaded`] when it equals the served name
/// or the checkpoint, else [`LocalError::ModelNotServed`]. While loading, the daemon does not
/// listen, so the probe fails with `http.network`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Strands;

/// The fields of a Strands Decider `/health` reply this crate reads.
#[derive(Deserialize)]
struct StrandsHealth {
    model: String,
    #[serde(default)]
    checkpoint: Option<String>,
}

impl sealed::Sealed for Strands {}

impl LocalDaemon for Strands {
    fn kind(&self) -> DaemonKind {
        DaemonKind::Strands
    }

    /// `GET {base}/health`; Strands Decider has no auth, so `key` is not sent.
    fn probe(&self, base: &BaseUrl, _key: Option<&ApiKey>) -> Result<HttpRequest, LocalError> {
        get(base, HEALTH_PATH, None)
    }

    fn availability(
        &self,
        response: &HttpResponse,
        model: &ModelId,
    ) -> Result<Availability, LocalError> {
        let daemon = self.kind();
        if response.status() != 200 {
            return Err(probe_status(daemon, HEALTH_PATH, response));
        }
        let health: StrandsHealth = parse(daemon, HEALTH_PATH, response)?;
        let loaded: Vec<String> = std::iter::once(health.model)
            .chain(health.checkpoint)
            .collect();
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
}
