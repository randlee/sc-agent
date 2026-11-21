//! [`Laya`]: discovery for Laya (`GET /health`, optional Bearer).

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

/// Laya: loads checkpoints on demand; Bearer on every route except liveness when
/// `LAYA_API_KEY` is set.
///
/// `GET /health` always answers 200. With the key (or no key configured on the daemon) it
/// lists `loaded` checkpoints: the configured model in the list is [`ModelState::Loaded`],
/// otherwise [`ModelState::LoadsOnDemand`]. Without a valid key the reply is only
/// `{"status":"ok"}`: [`ModelState::Unlisted`].
#[derive(Debug, Clone, Copy, Default)]
pub struct Laya;

/// The fields of a Laya `/health` reply this crate reads.
#[derive(Deserialize)]
struct LayaHealth {
    #[serde(default)]
    loaded: Option<Vec<String>>,
}

impl sealed::Sealed for Laya {}

impl LocalDaemon for Laya {
    fn kind(&self) -> DaemonKind {
        DaemonKind::Laya
    }

    /// `GET {base}/health`, with the Bearer key when one is configured.
    fn probe(&self, base: &BaseUrl, key: Option<&ApiKey>) -> Result<HttpRequest, LocalError> {
        get(base, HEALTH_PATH, key)
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
        let health: LayaHealth = parse(daemon, HEALTH_PATH, response)?;
        Ok(match health.loaded {
            Some(loaded) => {
                let state = if loaded.iter().any(|name| name == model.as_str()) {
                    ModelState::Loaded
                } else {
                    ModelState::LoadsOnDemand
                };
                Availability {
                    daemon,
                    loaded,
                    model: state,
                }
            }
            None => Availability {
                daemon,
                loaded: Vec::new(),
                model: ModelState::Unlisted,
            },
        })
    }
}
