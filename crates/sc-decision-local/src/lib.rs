#![forbid(unsafe_code)]
//! Local runners: Jev-compatible daemons on host:port. Each kind differs only in how it
//! reports whether a model is loaded, so that is all a [`LocalDaemon`] says.

use std::time::{Duration, Instant};

use sc_agent_types::Error;
use sc_agent_http::{Cancel, Client, Codec, Policy, http};
use serde::Deserialize;

use sc_decision_runner::{Decider, Decision, Outcome};
use sc_decision_schema::DecisionRequest;
use sc_decision_systemone::SystemOne;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Availability {
    Loaded,
    /// Loads on the first request (Laya).
    LoadsOnDemand,
    Loading,
    /// Serving a different model.
    Other(Vec<String>),
}

/// One kind of local runner.
pub trait LocalDaemon: Send + Sync {
    /// The probe path, from the base URL.
    fn probe_path(&self) -> &'static str {
        "/health"
    }
    /// What a 200 probe body says about `model`.
    fn availability(&self, body: &[u8], model: &str) -> Result<Availability, Error>;
}

fn parse<'a, T: Deserialize<'a>>(body: &'a [u8]) -> Result<T, Error> {
    serde_json::from_slice(body).map_err(|e| Error::ResponseParse(e.to_string()))
}

pub struct Laya;
impl LocalDaemon for Laya {
    fn availability(&self, body: &[u8], model: &str) -> Result<Availability, Error> {
        #[derive(Deserialize)]
        struct Health {
            loaded: Option<Vec<String>>,
        }
        let loaded = parse::<Health>(body)?.loaded.unwrap_or_default();
        Ok(if loaded.iter().any(|m| m == model) { Availability::Loaded } else { Availability::LoadsOnDemand })
    }
}

pub struct Strands;
impl LocalDaemon for Strands {
    fn availability(&self, body: &[u8], model: &str) -> Result<Availability, Error> {
        #[derive(Deserialize)]
        struct Health {
            model: String,
        }
        let served = parse::<Health>(body)?.model;
        Ok(if served == model { Availability::Loaded } else { Availability::Other(vec![served]) })
    }
}

pub struct Mapika;
impl LocalDaemon for Mapika {
    fn availability(&self, body: &[u8], model: &str) -> Result<Availability, Error> {
        #[derive(Deserialize)]
        struct Health {
            ok: bool,
            model: Option<String>,
        }
        let health = parse::<Health>(body)?;
        Ok(match health.model {
            Some(m) if m == model && health.ok => Availability::Loaded,
            Some(m) if m != model => Availability::Other(vec![m]),
            _ => Availability::Loading,
        })
    }
}

/// Any daemon serving the Jev model list.
pub struct Generic;
impl LocalDaemon for Generic {
    fn probe_path(&self) -> &'static str {
        "/v1/models"
    }
    fn availability(&self, body: &[u8], model: &str) -> Result<Availability, Error> {
        #[derive(Deserialize)]
        struct Entry {
            name: String,
        }
        #[derive(Deserialize)]
        struct List {
            models: Vec<Entry>,
        }
        let names: Vec<String> = parse::<List>(body)?.models.into_iter().map(|e| e.name).collect();
        Ok(if names.iter().any(|n| n == model) { Availability::Loaded } else { Availability::Other(names) })
    }
}

pub fn daemon(kind: &str) -> Result<Box<dyn LocalDaemon>, Error> {
    Ok(match kind {
        "laya" => Box::new(Laya),
        "strands" => Box::new(Strands),
        "mapika" => Box::new(Mapika),
        "generic" => Box::new(Generic),
        other => return Err(Error::Config(format!("unknown local runner kind {other}"))),
    })
}

/// The probe as a codec, so it goes through the same client as every other call.
struct Probe {
    url: String,
    daemon: Box<dyn LocalDaemon>,
}

impl Codec for Probe {
    type Request = String;
    type Response = Availability;

    fn encode(&self, _model: &String) -> Result<http::Request<Vec<u8>>, Error> {
        sc_agent_http::get(&self.url, None)
    }

    fn decode(&self, model: &String, status: u16, body: &[u8]) -> Result<Availability, Error> {
        match status {
            200 => self.daemon.availability(body, model),
            _ => Err(Error::ApiStatus { status, body: String::from_utf8_lossy(body).into_owned() }),
        }
    }
}

/// A local runner: waits for the model to load, then decides through the Jev API.
pub struct Local {
    probe: Client<Probe>,
    decider: Decider<SystemOne>,
    load_timeout: Duration,
    poll: Duration,
}

impl Local {
    pub fn new(base: &str, kind: &str, decider: impl FnOnce(SystemOne) -> Decider<SystemOne>, load_timeout: Duration) -> Result<Self, Error> {
        let base = sc_agent_http::check_base(base, false)?;
        let daemon = daemon(kind)?;
        let probe = Probe { url: format!("{base}{}", daemon.probe_path()), daemon };
        let policy = Policy { max_sends: 1, timeout: Duration::from_secs(10), ..Policy::default() };
        Ok(Self { probe: Client::new(probe, policy), decider: decider(SystemOne::new(&base, None)?), load_timeout, poll: Duration::from_secs(1) })
    }

    /// Probes until the model is loaded (or loads on demand), the load timeout passes, or
    /// cancel. The wait between probes is cancellable; nothing sleeps.
    pub fn wait_loaded(&self, model: &str, cancel: &Cancel) -> Result<(), Error> {
        let deadline = Instant::now() + self.load_timeout;
        loop {
            let model = model.to_owned();
            match self.probe.send(&model, cancel)? {
                Availability::Loaded | Availability::LoadsOnDemand => return Ok(()),
                Availability::Other(served) => {
                    return Err(Error::Config(format!("runner serves {served:?}, not {model}")));
                }
                Availability::Loading => {}
            }
            if Instant::now() >= deadline {
                return Err(Error::Config(format!("{model} did not load within {:?}", self.load_timeout)));
            }
            if cancel.wait(self.poll) {
                return Err(Error::Cancelled { sends: 0 });
            }
        }
    }
}

impl Decision for Local {
    fn decide(&self, request: DecisionRequest, cancel: &Cancel) -> Outcome {
        match self.wait_loaded(&request.model, cancel) {
            Ok(()) => self.decider.decide(request, cancel),
            Err(error) => Outcome::failed(0, request, error),
        }
    }

    fn decide_batch(&self, requests: Vec<DecisionRequest>, workers: usize, cancel: &Cancel, on_result: &mut dyn FnMut(Outcome)) {
        let first = requests.first().map(|r| r.model.clone()).unwrap_or_default();
        match self.wait_loaded(&first, cancel) {
            Ok(()) => self.decider.decide_batch(requests, workers, cancel, on_result),
            Err(error) => requests.into_iter().enumerate().for_each(|(i, r)| on_result(Outcome::failed(i, r, error.clone()))),
        }
    }
}

