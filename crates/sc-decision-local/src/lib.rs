#![forbid(unsafe_code)]
//! Laya local runner: probe availability before sending through the Jev API.

use std::time::Duration;

use sc_agent_http::{Cancel, Client, Codec, Policy, http};
use sc_agent_types::Error;
use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Availability {
    Loaded,
    /// Loads on the first request (Laya).
    LoadsOnDemand,
}

pub struct Laya;
impl Laya {
    pub fn availability(&self, body: &[u8], model: &str) -> Result<Availability, Error> {
        #[derive(Deserialize)]
        struct Health {
            loaded: Option<Vec<String>>,
        }
        let health: Health =
            serde_json::from_slice(body).map_err(|e| Error::ResponseParse(e.to_string()))?;
        let loaded = health.loaded.unwrap_or_default();
        Ok(if loaded.iter().any(|m| m == model) {
            Availability::Loaded
        } else {
            Availability::LoadsOnDemand
        })
    }
}

/// The probe as a codec, so it goes through the same client as every other call.
struct Probe {
    url: String,
}

impl Codec for Probe {
    type Request = String;
    type Response = Availability;

    fn encode(&self, _model: &String) -> Result<http::Request<Vec<u8>>, Error> {
        sc_agent_http::get(&self.url, None)
    }

    fn decode(&self, model: &String, status: u16, body: &[u8]) -> Result<Availability, Error> {
        match status {
            200 => Laya.availability(body, model),
            _ => Err(Error::ApiStatus {
                status,
                body: String::from_utf8_lossy(body).into_owned(),
            }),
        }
    }
}

/// A local runner: waits for the model to load, then decides through the Jev API.
pub struct Local {
    probe: Client<Probe>,
}

impl Local {
    pub fn new(base: &str, kind: &str, _load_timeout: Duration) -> Result<Self, Error> {
        let base = sc_agent_http::check_base(base, false)?;
        if kind != "laya" {
            return Err(Error::Config(format!("unknown local runner kind {kind}")));
        }
        let probe = Probe {
            url: format!("{base}/health"),
        };
        let policy = Policy {
            max_sends: 1,
            timeout: Duration::from_secs(10),
            ..Policy::default()
        };
        Ok(Self {
            probe: Client::new(probe, policy),
        })
    }

    /// Probes once; Laya either has the model loaded or loads it on the first request.
    pub fn wait_loaded(&self, model: &str, cancel: &Cancel) -> Result<Availability, Error> {
        self.probe.send(&model.to_owned(), cancel)
    }
}
