//! [`Mapika`]: discovery for Mapika/decider (`GET /health`).

use sc_agent_http::{HttpRequest, HttpResponse};
use sc_agent_secrets::ApiKey;
use sc_agent_types::ModelId;
use serde::Deserialize;

use crate::daemon::{
    Availability, DaemonKind, HEALTH_PATH, LocalDaemon, ModelState, get, parse, probe_status,
    sealed,
};
use crate::error::LocalError;

/// Mapika/decider: one model per process, no auth, loads at start.
///
/// `GET /health` answers 200 `{"ok": bool, "model": <name>, ...}`. `ok: false` is
/// [`ModelState::NotReady`]; `ok: true` with the configured model is [`ModelState::Loaded`];
/// `ok: true` with another model is [`LocalError::ModelNotServed`].
#[derive(Debug, Clone, Copy, Default)]
pub struct Mapika;

/// The fields of a Mapika/decider `/health` reply this crate reads.
#[derive(Deserialize)]
struct MapikaHealth {
    ok: bool,
    #[serde(default)]
    model: Option<String>,
}

impl sealed::Sealed for Mapika {}

impl LocalDaemon for Mapika {
    fn kind(&self) -> DaemonKind {
        DaemonKind::Mapika
    }

    /// `GET {base}/health`; Mapika/decider has no auth, so `key` is not sent.
    fn probe(&self, base: &str, _key: Option<&ApiKey>) -> Result<HttpRequest, LocalError> {
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
        let health: MapikaHealth = parse(daemon, HEALTH_PATH, response)?;
        let served: Vec<String> = health.model.into_iter().collect();
        if !health.ok {
            return Ok(Availability {
                daemon,
                loaded: Vec::new(),
                model: ModelState::NotReady,
            });
        }
        if served.iter().any(|name| name == model.as_str()) {
            Ok(Availability {
                daemon,
                loaded: served,
                model: ModelState::Loaded,
            })
        } else {
            Err(LocalError::ModelNotServed {
                daemon,
                model: model.clone(),
                served,
            })
        }
    }
}
