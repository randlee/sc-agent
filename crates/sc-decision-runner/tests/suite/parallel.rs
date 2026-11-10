//! REQ-SAG-0701 AC1-AC3, REQ-SAG-0013: parallel workers, serial with one, failures returned,
//! fastest first, config.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::sync::{Arc, Condvar, Mutex, PoisonError};

use sc_agent_config::AgentConfig;
use sc_agent_http::HttpResponse;
use sc_agent_types::{CancelToken, ErrorId};
use sc_decision_runner::{RunnerConfig, RunnerError};
use sc_decision_schema::Decision;

use crate::support::*;

/// Counts transport calls in flight and the most seen at once; the first `parties` calls
/// meet at a barrier (each returns only once `parties` calls are inside together).
struct InFlight {
    parties: usize,
    /// (in flight now, most at once, calls so far, first `parties` calls that met).
    state: Mutex<(usize, usize, usize, usize)>,
    /// Calls in flight at each call's entry, in entry order (that call included).
    at_entry: Mutex<Vec<usize>>,
    changed: Condvar,
}

impl InFlight {
    fn new(parties: usize) -> Self {
        Self {
            parties,
            state: Mutex::new((0, 0, 0, 0)),
            at_entry: Mutex::new(Vec::new()),
            changed: Condvar::new(),
        }
    }

    fn enter(&self) {
        let mut state = lock(&self.state);
        state.0 += 1;
        state.1 = state.1.max(state.0);
        state.2 += 1;
        lock(&self.at_entry).push(state.0);
        if state.2 > self.parties {
            return;
        }
        // Rendezvous: wait until `parties` calls are inside at once.
        self.changed.notify_all();
        let (mut state, timeout) = self
            .changed
            .wait_timeout_while(state, FAILSAFE, |state| {
                state.0 < self.parties && state.1 < self.parties
            })
            .unwrap_or_else(PoisonError::into_inner);
        if !timeout.timed_out() {
            state.3 += 1;
        }
    }

    fn leave(&self) {
        lock(&self.state).0 -= 1;
    }

    /// (most in flight at once, how many of the first `parties` calls met).
    fn seen(&self) -> (usize, usize) {
        let state = lock(&self.state);
        (state.1, state.3)
    }

    /// Calls in flight at each entry.
    fn at_entry(&self) -> Vec<usize> {
        lock(&self.at_entry).clone()
    }
}

/// Runs 40 requests, the first `parties` transport calls at a barrier; returns the counter.
/// No delay and no clock: concurrency is shown by who was inside together.
fn forty(config: RunnerConfig, parties: usize) -> Arc<InFlight> {
    let in_flight = Arc::new(InFlight::new(parties));
    let counted = Arc::clone(&in_flight);
    let (transport, _dropped) = FnTransport::new(move |_| {
        counted.enter();
        counted.leave();
        Ok(ok())
    });
    let harness = Harness::new(
        Arc::new(ScriptedBackend::new()),
        Arc::new(transport),
        config,
        5,
    );
    let results: Vec<_> = harness
        .runner
        .decide_stream(Box::new(requests(40).into_iter()), &CancelToken::new())
        .collect();
    assert_eq!(results.len(), 40);
    assert!(results.iter().all(|result| result.outcome.is_ok()));
    let positions: BTreeSet<u64> = results.iter().map(|result| result.position).collect();
    assert_eq!(positions, (0..40).collect());
    in_flight
}

/// REQ-SAG-0701 AC1: 40 requests with the default 8 workers: the first 8 transport calls
/// meet at a Rendezvous(8) barrier, so 8 are in flight at once, and never more.
#[test]
fn forty_requests_parallel() {
    let in_flight = forty(RunnerConfig::default(), 8);
    let (most, met) = in_flight.seen();
    assert_eq!(met, 8, "the first 8 calls were in flight together");
    assert_eq!(most, 8, "never more than 8 at once");
}

/// REQ-SAG-0701 AC2: one worker runs one request at a time: every call finds itself alone.
#[test]
fn one_worker_is_serial() {
    let in_flight = forty(config(1), 1);
    let at_entry = in_flight.at_entry();
    assert_eq!(at_entry.len(), 40);
    assert!(
        at_entry.iter().all(|count| *count == 1),
        "one worker: one call in flight at each entry ({at_entry:?})"
    );
}

/// REQ-SAG-0701 AC2 / REQ-SAG-0013: workers from AgentConfig.workers (default 8),
/// queue_capacity = 4 x workers; out-of-bounds workers are a typed error.
#[test]
fn config_from_agent_config() {
    let mut agent = AgentConfig::default();
    let default = RunnerConfig::from_config(&agent).expect("default");
    assert_eq!(
        (default.workers().get(), default.queue_capacity().get()),
        (8, 32)
    );
    assert_eq!(RunnerConfig::default(), default);
    agent.workers = NonZeroUsize::new(64).expect("64");
    let most = RunnerConfig::from_config(&agent).expect("64 workers");
    assert_eq!(
        (most.workers().get(), most.queue_capacity().get()),
        (64, 256)
    );
    agent.workers = NonZeroUsize::MIN;
    let one = RunnerConfig::from_config(&agent).expect("1 worker");
    assert_eq!((one.workers().get(), one.queue_capacity().get()), (1, 4));
    agent.workers = NonZeroUsize::new(65).expect("65");
    let error = RunnerConfig::from_config(&agent).expect_err("65 is over the bound");
    assert!(matches!(
        error,
        RunnerError::InvalidConfig {
            workers: 65,
            max: 64
        }
    ));
    assert_eq!(error.id(), "runner.invalid_config");
    assert_eq!(
        error.to_string(),
        "runner.invalid_config: workers must be 1..=64, got 65"
    );
    assert!(RunnerConfig::new(0).is_err());
    assert!(RunnerConfig::new(usize::MAX).is_err());
}

/// REQ-SAG-0701 AC3: one failing request is returned with its session ULID; every other
/// result is returned, each with its session ULID and input position.
#[test]
fn failed_request_returned_rest_continue() {
    let (transport, _dropped) = FnTransport::new(|request| {
        if index_of(request) == 3 {
            Ok(HttpResponse::new(500, vec![], b"internal error".to_vec()))
        } else {
            Ok(ok())
        }
    });
    let harness = Harness::of(Arc::new(transport));
    let results: Vec<_> = harness
        .runner
        .decide_stream(Box::new(requests(10).into_iter()), &CancelToken::new())
        .collect();
    assert_eq!(results.len(), 10);
    let positions: BTreeSet<u64> = results.iter().map(|result| result.position).collect();
    assert_eq!(positions, (0..10).collect());
    let sessions: BTreeSet<String> = results
        .iter()
        .map(|result| result.session_id.to_string())
        .collect();
    assert_eq!(sessions.len(), 10, "one session ULID per request");
    for result in &results {
        assert_eq!(result.outcome.is_err(), result.position == 3, "{result:?}");
    }
    let rows = harness.rows();
    let failed = results
        .iter()
        .find(|result| result.position == 3)
        .expect("failed result");
    let failed_row = rows
        .iter()
        .find(|row| row.fields["position"] == 3)
        .expect("row of the failed request");
    assert_eq!(
        failed_row.fields["session_id"],
        failed.session_id.to_string()
    );
    assert_eq!(failed_row.fields["outcome"], "error");
    assert_eq!(failed_row.fields["error"]["id"], "decision.response_parse");
}

/// REQ-SAG-0701 AC3: results come back in finish order.
#[test]
fn fastest_first() {
    let gate = Gate::default();
    let held = gate.clone();
    let (transport, _dropped) = FnTransport::new(move |request| {
        if index_of(request) == 0 {
            held.pass();
        }
        Ok(ok())
    });
    let harness = Harness::of(Arc::new(transport));
    let results = harness.stream(requests(2), &CancelToken::new());
    let first = take(&results, 1);
    assert_eq!(
        first[0].position, 1,
        "the request that finished first comes first"
    );
    gate.open();
    let second = take(&results, 1);
    assert_eq!(second[0].position, 0);
    assert!(drain(&results).is_empty());
}
