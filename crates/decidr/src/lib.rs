#![forbid(unsafe_code)]
//! decidr's composition root: picks the provider and key, builds the backend once, and
//! returns a [`Decision`]. The CLI and the Python module both start here.

use sc_agent_http::{Cancel, Policy};
use sc_agent_log::Session;
use sc_agent_types::Error;
use sc_decision_runner::{Decider, Decision, Outcome};
use sc_decision_schema::{Backend, DecisionRequest};
use std::time::Duration;

struct LocalDecision {
    probe: sc_decision_local::Local,
    decider: Decider<sc_decision_systemone::SystemOne>,
    loading: Decider<sc_decision_systemone::SystemOne>,
}

impl Decision for LocalDecision {
    fn decide(&self, request: DecisionRequest, cancel: &Cancel) -> Outcome {
        match self.probe.wait_loaded(&request.model, cancel) {
            Ok(sc_decision_local::Availability::LoadsOnDemand) => {
                self.loading.decide(request, cancel)
            }
            Ok(_) => self.decider.decide(request, cancel),
            Err(error) => Outcome::failed(0, request, error),
        }
    }

    fn decide_batch(
        &self,
        mut requests: Vec<DecisionRequest>,
        workers: usize,
        cancel: &Cancel,
        on_result: &mut dyn FnMut(Outcome),
    ) {
        let model = requests
            .first()
            .map(|r| r.model.as_str())
            .unwrap_or_default();
        let mut offset = 0;
        match self.probe.wait_loaded(model, cancel) {
            Ok(sc_decision_local::Availability::LoadsOnDemand) => {
                while !requests.is_empty() {
                    let mut first = self.loading.decide(requests.remove(0), cancel);
                    first.position = offset;
                    let loaded = matches!(first.result, Ok(_) | Err(Error::ResponseParse(_)));
                    on_result(first);
                    offset += 1;
                    if loaded {
                        break;
                    }
                }
            }
            Ok(_) => {}
            Err(error) => {
                requests
                    .into_iter()
                    .enumerate()
                    .for_each(|(i, r)| on_result(Outcome::failed(i, r, error.clone())));
                return;
            }
        }
        self.decider
            .decide_batch(requests, workers, cancel, &mut |mut outcome| {
                outcome.position += offset;
                on_result(outcome);
            });
    }
}

pub fn open_local(
    base: &str,
    kind: &str,
    policy: Policy,
    session: Option<Session>,
    load_timeout: std::time::Duration,
) -> Result<Box<dyn Decision>, Error> {
    Ok(Box::new(LocalDecision {
        probe: sc_decision_local::Local::new(base, kind, load_timeout)?,
        loading: Decider::new(
            sc_decision_systemone::SystemOne::new(base, None)?,
            Policy {
                timeout: policy.timeout + load_timeout,
                ..policy
            },
            session.clone(),
        ),
        decider: Decider::new(
            sc_decision_systemone::SystemOne::new(base, None)?,
            policy,
            session,
        ),
    }))
}

/// The decision backend for a provider: the Jev API itself, or a chat model through
/// [`FromLlm`]. The key and base URL are checked here, once.
pub fn backend(
    provider: &sc_agent_types::providers::Provider,
    key: Option<String>,
) -> Result<Box<dyn Backend>, Error> {
    match provider.wire {
        sc_agent_types::providers::Wire::SystemOne => Ok(Box::new(
            sc_decision_systemone::SystemOne::new(provider.base_url, key.as_deref())?,
        )),
        _ => Ok(Box::new(sc_decision_llm::FromLlm::new(
            sc_llm_provider::backend(provider, key)?,
        ))),
    }
}

/// A decider for `model` (short name or id) on `provider`, or the first provider in ranking
/// order with a key. Returns it with the provider's model id, which goes into each request.
pub fn open(
    model: &str,
    provider: Option<&str>,
    repo_root: Option<&std::path::Path>,
    policy: Policy,
    session: Option<Session>,
    keyring: Option<(Option<&str>, Option<&str>)>,
) -> Result<(Box<dyn Decision>, String), Error> {
    let key = |p: &sc_agent_types::providers::Provider| {
        sc_agent_secrets::key(p.key_var, repo_root, keyring)
    };
    let (chosen, id) = sc_agent_types::providers::Provider::choose(model, provider, |p| {
        matches!(key(p), Ok(Some(_)))
    })?;
    let found = key(chosen)?.map(|(value, _)| value);
    let found = Some(found.ok_or_else(|| Error::MissingKey {
        provider: chosen.name.into(),
        var: chosen.key_var.into(),
    })?);
    Ok((
        Box::new(Decider::new(backend(chosen, found)?, policy, session)),
        id,
    ))
}

/// Resolve the configured policy once for CLI and wheel callers; overrides are already validated.
pub fn policy(
    config: &sc_agent_config::Config,
    workers: Option<usize>,
    timeout: Option<Duration>,
    runner: Option<&sc_agent_config::Runner>,
) -> (Policy, usize) {
    (
        Policy {
            max_sends: config.max_sends.unwrap_or(5),
            timeout: timeout.unwrap_or(Duration::from_secs(
                runner.map_or(config.request_timeout_secs.unwrap_or(120), |r| {
                    r.request_timeout_secs
                }),
            )),
            ..Policy::default()
        },
        workers.or(config.workers).unwrap_or(8),
    )
}

/// Validate per-call overrides once. None leaves the caller's configured value intact.
pub fn validate_limits(
    timeout: Option<f64>,
    workers: Option<i64>,
) -> Result<(Option<Duration>, Option<usize>), Error> {
    if let Some(value) = timeout
        && (!value.is_finite() || !(10.0..=3600.0).contains(&value))
    {
        return Err(Error::InvalidRequestTimeout(value));
    }
    if let Some(value) = workers
        && !(1..=64).contains(&value)
    {
        return Err(Error::InvalidWorkers(value));
    }
    Ok((
        timeout.map(Duration::from_secs_f64),
        workers.map(|n| n as usize),
    ))
}
