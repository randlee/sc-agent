//! Composes local daemon readiness with the shared decision runner.
use std::time::Duration;

use sc_agent_http::Cancel;
use sc_agent_types::Error;
use sc_decision_runner::{Decider, Decision, Outcome};
use sc_decision_schema::DecisionRequest;
use sc_decision_systemone::SystemOne;

/// A local runner: waits for its model, then decides through the Jev API.
pub struct Local {
    probe: sc_decision_local::Local,
    decider: Decider<SystemOne>,
}

impl Local {
    pub fn new(
        base: &str,
        kind: &str,
        decider: impl FnOnce(SystemOne) -> Decider<SystemOne>,
        load_timeout: Duration,
    ) -> Result<Self, Error> {
        Ok(Self {
            probe: sc_decision_local::Local::new(base, kind, load_timeout)?,
            decider: decider(SystemOne::new(base, None)?),
        })
    }
}

impl Decision for Local {
    fn decide(&self, request: DecisionRequest, cancel: &Cancel) -> Outcome {
        match self.probe.wait_loaded(&request.model, cancel) {
            Ok(()) => self.decider.decide(request, cancel),
            Err(error) => Outcome::failed(0, request, error),
        }
    }

    fn decide_batch(
        &self,
        requests: Vec<DecisionRequest>,
        workers: usize,
        cancel: &Cancel,
        on_result: &mut dyn FnMut(Outcome),
    ) {
        let first = requests
            .first()
            .map(|r| r.model.clone())
            .unwrap_or_default();
        match self.probe.wait_loaded(&first, cancel) {
            Ok(()) => self
                .decider
                .decide_batch(requests, workers, cancel, on_result),
            Err(error) => requests
                .into_iter()
                .enumerate()
                .for_each(|(i, r)| on_result(Outcome::failed(i, r, error.clone()))),
        }
    }
}
