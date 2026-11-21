#![forbid(unsafe_code)]
//! decidr's composition root: picks the provider and key, builds the backend once, and
//! returns a [`Decision`]. The CLI and the Python module both start here.

use sc_agent_http::{Cancel, Policy};
use sc_agent_log::{Session, record};
use sc_agent_types::Error;
use sc_decision_runner::{Decider, Decision, Outcome, digests};
use sc_decision_schema::{Backend, DecisionRequest};
use sc_observability::v2::Logger;
use serde_json::Map;
use std::time::Duration;

/// The first request may load the model; later requests use the normal policy.
pub fn load_policy(policy: Policy, load: Duration) -> Policy {
    Policy {
        timeout: policy.timeout + load,
        ..policy
    }
}

/// One execution path for CLI and Python: resolve, choose the backend, stamp models, run.
#[expect(
    clippy::too_many_arguments,
    reason = "shared CLI and Python call inputs"
)]
pub fn send(
    mut requests: Vec<DecisionRequest>,
    model: &str,
    provider: Option<&str>,
    runner: Option<&str>,
    config: &sc_agent_config::Config,
    workers: Option<usize>,
    timeout: Option<Duration>,
    root: Option<&std::path::Path>,
    session: Option<Session>,
    keyring: Option<(Option<&str>, Option<&str>)>,
    logger: &Logger,
    cancel: &Cancel,
    on_result: &mut dyn FnMut(&mut Outcome),
) -> Result<(), Error> {
    let runner =
        runner.or_else(|| provider.filter(|name| config.local_runners.contains_key(*name)));
    let local = runner
        .map(|name| {
            config
                .local_runners
                .get(name)
                .ok_or_else(|| Error::Config(format!("no local runner {name} in config")))
        })
        .transpose()?;
    let policy = Policy {
        max_sends: config.max_sends.unwrap_or(5),
        timeout: timeout.unwrap_or(Duration::from_secs(
            local.map_or(120, |r| r.request_timeout_secs),
        )),
        ..Policy::default()
    };
    let workers = workers.or(config.workers).unwrap_or(8);
    let (backend, id, loading): (Box<dyn Backend>, String, _) = match local {
        Some(r) => {
            let base = format!("http://{}:{}", r.host, r.port);
            let id = r.model.as_deref().unwrap_or(model).to_owned();
            let loading =
                match sc_decision_local::Local::new(&base, &r.kind)?.wait_loaded(&id, cancel) {
                    Ok(sc_decision_local::Availability::LoadsOnDemand) => Some(Ok(Decider::new(
                        sc_decision_systemone::SystemOne::new(&base, None)?,
                        load_policy(policy, Duration::from_secs(r.load_timeout_secs)),
                        session.clone(),
                    ))),
                    Ok(_) => None,
                    Err(e) => Some(Err(e)),
                };
            (
                Box::new(sc_decision_systemone::SystemOne::new(&base, None)?) as Box<dyn Backend>,
                id,
                loading,
            )
        }
        None => {
            let key = |p: &sc_agent_types::providers::Provider| {
                sc_agent_secrets::key(p.key_var, root, keyring)
            };
            let (chosen, id) = sc_agent_types::providers::Provider::choose(model, provider, |p| {
                matches!(key(p), Ok(Some(_)))
            })?;
            let found =
                Some(
                    key(chosen)?
                        .map(|(value, _)| value)
                        .ok_or_else(|| Error::MissingKey {
                            provider: chosen.name.into(),
                            var: chosen.key_var.into(),
                        })?,
                );
            let backend: Box<dyn Backend> = match chosen.wire {
                sc_agent_types::providers::Wire::SystemOne => Box::new(
                    sc_decision_systemone::SystemOne::new(chosen.base_url, found.as_deref())?,
                ),
                _ => Box::new(sc_decision_llm::FromLlm::new(sc_llm_provider::backend(
                    chosen, found,
                )?)),
            };
            (backend, id, None)
        }
    };
    let decision = Decider::new(backend, policy, session);
    for request in &mut requests {
        request.model.clone_from(&id);
    }
    let mut finish = |mut outcome: Outcome| {
        if let Ok(response) = &mut outcome.result {
            response
                .extra
                .insert("digests".into(), digests(&outcome.request).into());
        }
        on_result(&mut outcome);
        let mut fields = Map::new();
        fields.insert("model".into(), outcome.request.model.clone().into());
        fields.insert("position".into(), outcome.position.into());
        fields.insert("session_id".into(), outcome.session.to_string().into());
        fields.insert("ok".into(), outcome.result.is_ok().into());
        fields.insert(
            "error_id".into(),
            outcome.result.as_ref().err().map(|e| e.id()).into(),
        );
        record(logger, "sc_agent.decidr", "decidr.send", fields);
    };
    let mut offset = 0;
    if let Some(Err(error)) = &loading {
        requests
            .into_iter()
            .enumerate()
            .for_each(|(position, request)| {
                finish(sc_decision_runner::failed(position, request, error.clone()))
            });
        return Ok(());
    }
    if let Some(Ok(loading)) = loading {
        while !requests.is_empty() {
            let mut first = loading.decide(requests.remove(0), cancel);
            first.position = offset;
            let loaded = matches!(first.result, Ok(_) | Err(Error::ResponseParse(_)));
            finish(first);
            offset += 1;
            if loaded {
                break;
            }
        }
    }
    decision.decide_batch(requests, workers, cancel, &mut |mut outcome| {
        outcome.position += offset;
        finish(outcome);
    });
    Ok(())
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
