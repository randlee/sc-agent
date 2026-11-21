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
}

fn local_policy(policy: Policy, kind: &str, load_timeout: Duration) -> Policy {
    if kind == "laya" {
        Policy {
            timeout: load_timeout.saturating_add(policy.timeout),
            ..policy
        }
    } else {
        policy
    }
}

impl Decision for LocalDecision {
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
        let model = requests
            .first()
            .map(|r| r.model.as_str())
            .unwrap_or_default();
        match self.probe.wait_loaded(model, cancel) {
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

pub fn open_local(
    base: &str,
    kind: &str,
    policy: Policy,
    session: Option<Session>,
    load_timeout: std::time::Duration,
) -> Result<Box<dyn Decision>, Error> {
    let policy = local_policy(policy, kind, load_timeout);
    Ok(Box::new(LocalDecision {
        probe: sc_decision_local::Local::new(base, kind, load_timeout)?,
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

#[cfg(test)]
mod tests {
    use super::local_policy;
    use sc_agent_http::Policy;
    use std::time::Duration;

    /// REQ-SAG-0702: Laya can report `Availability::LoadsOnDemand`, so its send allowance
    /// is the model load timeout plus the selected request timeout.
    #[test]
    fn laya_local_runner_timeout_includes_load_allowance() {
        let policy = Policy {
            max_sends: 3,
            backoff: Duration::ZERO,
            timeout: Duration::from_secs(10),
        };

        assert_eq!(
            local_policy(policy, "laya", Duration::from_secs(17)).timeout,
            Duration::from_secs(27),
        );
        assert_eq!(
            local_policy(policy, "strands", Duration::from_secs(17)).timeout,
            Duration::from_secs(10),
        );
    }
}
