//! REQ-SAG-0701 AC1-AC3, REQ-SAG-0013: parallel workers, serial with one, failures returned,
//! fastest first, config.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sc_agent_config::AgentConfig;
use sc_agent_http::HttpResponse;
use sc_agent_types::CancelToken;
use sc_decision_runner::RunnerConfig;
use sc_decision_schema::Decision;

use crate::support::*;

/// Counts transport calls in flight and the most seen at once.
#[derive(Default)]
struct InFlight {
    now_and_max: Mutex<(usize, usize)>,
}

impl InFlight {
    fn enter(&self) {
        let mut state = lock(&self.now_and_max);
        state.0 += 1;
        state.1 = state.1.max(state.0);
    }

    fn leave(&self) {
        lock(&self.now_and_max).0 -= 1;
    }

    fn max(&self) -> usize {
        lock(&self.now_and_max).1
    }
}

/// Runs 40 requests answered after `each`, returns (elapsed, most in flight).
fn forty(config: RunnerConfig, each: Duration) -> (Duration, usize) {
    let in_flight = Arc::new(InFlight::default());
    let counted = Arc::clone(&in_flight);
    let (transport, _dropped) = FnTransport::new(move |_| {
        counted.enter();
        delay(each);
        counted.leave();
        Ok(ok())
    });
    let harness = Harness::new(
        Arc::new(ScriptedBackend::new()),
        Arc::new(transport),
        config,
        5,
    );
    let started = Instant::now();
    let results: Vec<_> = harness
        .runner
        .decide_stream(Box::new(requests(40).into_iter()), &CancelToken::new())
        .collect();
    let elapsed = started.elapsed();
    assert_eq!(results.len(), 40);
    assert!(results.iter().all(|result| result.outcome.is_ok()));
    let positions: BTreeSet<u64> = results.iter().map(|result| result.position).collect();
    assert_eq!(positions, (0..40).collect());
    (elapsed, in_flight.max())
}

/// REQ-SAG-0701 AC1: 40 requests with the default 8 workers: 8 transport calls in flight
/// at once, finishing in about an eighth of the serial time.
#[test]
fn forty_requests_parallel() {
    let each = Duration::from_millis(50);
    let (elapsed, most) = forty(RunnerConfig::default(), each);
    assert_eq!(most, 8, "8 transport calls observed in flight at once");
    let serial = each * 40;
    // 5 rounds of 50 ms; the bound leaves room for a loaded machine.
    assert!(
        elapsed < serial / 2,
        "parallel run took {elapsed:?}, serial would be {serial:?}"
    );
}

/// REQ-SAG-0701 AC2: one worker runs one request at a time.
#[test]
fn one_worker_is_serial() {
    let each = Duration::from_millis(10);
    let (elapsed, most) = forty(config(1, 4), each);
    assert_eq!(most, 1, "one worker: one call at a time");
    assert!(elapsed >= each * 40, "serial run took {elapsed:?}");
}

/// REQ-SAG-0701 AC2 / REQ-SAG-0013: workers from AgentConfig.workers (default 8),
/// queue_capacity = 4 x workers.
#[test]
fn config_from_agent_config() {
    let mut agent = AgentConfig::default();
    let default = RunnerConfig::from_config(&agent);
    assert_eq!(
        (default.workers.get(), default.queue_capacity.get()),
        (8, 32)
    );
    assert_eq!(RunnerConfig::default(), default);
    agent.workers = NonZeroUsize::new(64).expect("64");
    let most = RunnerConfig::from_config(&agent);
    assert_eq!((most.workers.get(), most.queue_capacity.get()), (64, 256));
    agent.workers = NonZeroUsize::MIN;
    let one = RunnerConfig::from_config(&agent);
    assert_eq!((one.workers.get(), one.queue_capacity.get()), (1, 4));
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
