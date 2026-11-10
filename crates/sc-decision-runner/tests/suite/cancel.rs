//! REQ-SAG-0701 AC4, REQ-SAG-0104 AC1, REQ-SAG-0011 (runner side), cancel in flight,
//! token ownership.

use std::collections::BTreeSet;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex};

use sc_agent_http::Transport;
use sc_agent_http::testing::BlockingTransport;
use sc_agent_types::CancelToken;
use sc_decision_schema::{Decision, DecisionError};

use crate::support::*;

/// REQ-SAG-0701 AC4, REQ-SAG-0104 AC1, REQ-SAG-0011: a token created here, cancelled while
/// 8 sends are blocked, ends every unfinished request of 40 with decision.cancelled at
/// once (in flight, queued and not yet submitted); one result and one row per request.
#[test]
fn cancel_running_batch() {
    let gate = Gate::default();
    let held = gate.clone();
    let (transport, dropped) = FnTransport::new(move |_| {
        held.pass();
        Ok(ok())
    });
    let harness = Harness::of(Arc::new(transport));
    let cancel = CancelToken::new();
    let results = harness.stream(requests(40), &cancel);
    gate.wait_inside(8);
    cancel.cancel();
    let all = take(&results, 40);
    assert_eq!(
        gate.inside(),
        8,
        "every result arrived while the 8 sends were blocked"
    );
    let positions: BTreeSet<u64> = all.iter().map(|result| result.position).collect();
    assert_eq!(positions, (0..40).collect(), "one result per request");
    let mut in_flight = 0;
    for result in &all {
        match &result.outcome {
            Err(DecisionError::Cancelled { sends: 1 }) => in_flight += 1,
            Err(DecisionError::Cancelled { sends: 0 }) => {}
            other => panic!("position {}: {other:?}", result.position),
        }
    }
    assert_eq!(
        in_flight, 8,
        "the 8 blocked sends were cancelled after 1 send"
    );
    gate.open();
    assert!(
        drain(&results).is_empty(),
        "no result after the transport returns"
    );
    let log = harness.finish(&dropped);
    let rows = rows_of(&log);
    assert_eq!(rows.len(), 40, "one row per request");
    assert_eq!(rows_by_session(&rows).len(), 40);
}

/// Cancel in flight, REQ-SAG-0104 AC1 (decide_stream): the Cancelled result arrives while
/// the transport is still blocked; releasing it gives no further result and no second row.
#[test]
fn cancel_in_flight_send() {
    let blocking = BlockingTransport::new(Ok(ok()));
    let sender = blocking.clone();
    let (transport, dropped) = FnTransport::new(move |request| sender.send(request));
    let harness = Harness::of(Arc::new(transport));
    let cancel = CancelToken::new();
    let results = harness.stream(vec![request("m0")], &cancel);
    assert!(blocking.wait_started(1), "sends started");
    cancel.cancel();
    let result = results
        .recv_timeout(FAILSAFE)
        .expect("cancelled within 2 s");
    assert!(
        matches!(result.outcome, Err(DecisionError::Cancelled { sends: 1 })),
        "{result:?}"
    );
    assert_eq!(blocking.sends(), 1, "the transport is still blocked");
    blocking.release();
    assert_eq!(
        results.recv_timeout(FAILSAFE).err(),
        Some(RecvTimeoutError::Disconnected),
        "no further result"
    );
    let log = harness.finish(&dropped);
    let rows = rows_of(&log);
    assert_eq!(rows.len(), 1, "no second session row");
    assert_eq!(rows[0].fields["error"]["id"], "decision.cancelled");
    assert_eq!(rows[0].fields["sends"], 1);
}

/// Cancel in flight, REQ-SAG-0104 AC1 (decide): the same, through `decide`.
#[test]
fn decide_cancel_in_flight_send() {
    let blocking = BlockingTransport::new(Ok(ok()));
    let sender = blocking.clone();
    let (transport, dropped) = FnTransport::new(move |request| sender.send(request));
    let harness = Harness::of(Arc::new(transport));
    let cancel = CancelToken::new();
    let result = harness.decide(request("m0"), &cancel);
    assert!(blocking.wait_started(1), "sends started");
    cancel.cancel();
    let result = result.recv_timeout(FAILSAFE).expect("cancelled within 2 s");
    assert!(
        matches!(result.outcome, Err(DecisionError::Cancelled { sends: 1 })),
        "{result:?}"
    );
    assert_eq!(result.position, 0);
    assert_eq!(blocking.sends(), 1, "the transport is still blocked");
    blocking.release();
    let log = harness.finish(&dropped);
    assert_eq!(rows_of(&log).len(), 1, "no second session row");
}

/// Cancel in flight: decide's send runs on a runner worker, not the caller's thread.
#[test]
fn decide_runs_on_a_worker() {
    let names = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&names);
    let (transport, _dropped) = FnTransport::new(move |_| {
        lock(&seen).push(std::thread::current().name().map(str::to_owned));
        Ok(ok())
    });
    let harness = Harness::of(Arc::new(transport));
    let result = harness.runner.decide(request("m0"), &CancelToken::new());
    assert!(result.outcome.is_ok(), "{result:?}");
    assert_eq!(*lock(&names), vec![Some("sc-decision-worker".to_owned())]);
}

/// Token ownership: dropping the result iterator never cancels the caller's token.
#[test]
fn dropping_the_stream_does_not_cancel_the_token() {
    let (transport, _dropped) = FnTransport::new(|_| Ok(ok()));
    let harness = Harness::of(Arc::new(transport));
    let cancel = CancelToken::new();
    let mut stream = harness
        .runner
        .decide_stream(Box::new(requests(3).into_iter()), &cancel);
    assert!(stream.next().is_some());
    drop(stream);
    assert!(!cancel.is_cancelled());
    let result = harness.runner.decide(request("m9"), &cancel);
    assert!(
        result.outcome.is_ok(),
        "the token still works for the next run"
    );
}

/// QA-002: a token cancelled before the run: every request is answered cancelled and the
/// transport is never called, for decide_stream and decide. One deterministic run: the
/// ordering (hook registered before any runner thread starts) is proven by the crate-private
/// seam test `the_cancel_hook_is_registered_before_any_thread_starts`, not by repetition.
#[test]
fn cancelled_before_start_sends_nothing() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = Arc::clone(&calls);
    let (transport, dropped) = FnTransport::new(move |_| {
        counted.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        Ok(ok())
    });
    let harness = Harness::of(Arc::new(transport));
    let cancel = CancelToken::new();
    cancel.cancel();
    let results: Vec<_> = harness
        .runner
        .decide_stream(Box::new(requests(40).into_iter()), &cancel)
        .collect();
    assert_eq!(results.len(), 40);
    assert!(
        results
            .iter()
            .all(|result| matches!(result.outcome, Err(DecisionError::Cancelled { sends: 0 })))
    );
    let decided = harness.runner.decide(request("m99"), &cancel);
    assert!(matches!(
        decided.outcome,
        Err(DecisionError::Cancelled { sends: 0 })
    ));
    let log = harness.finish(&dropped);
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::Acquire),
        0,
        "nothing was sent"
    );
    assert_eq!(rows_of(&log).len(), 41);
}
