//! Abandon (RSH-006) and the result queue bound (RSH-011).

use std::collections::BTreeSet;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sc_agent_http::Transport;
use sc_agent_http::testing::BlockingTransport;
use sc_agent_types::CancelToken;
use sc_decision_schema::{Decision, DecisionError};

use crate::support::scripted_backend::default_encode;
use crate::support::*;

/// Abandon: 20 requests, 3 fail at encode, 8 sends blocked; dropping the iterator after
/// the 3 results stops intake, cancels the queue (not the token), joins nothing, and
/// writes exactly one row per job the feeder submitted.
#[test]
fn drop_mid_batch_one_row_per_started_job() {
    let blocking = BlockingTransport::new(Ok(ok()));
    let sender = blocking.clone();
    let (transport, dropped) = FnTransport::new(move |request| sender.send(request));
    let backend = ScriptedBackend::new().with_encode(|request| {
        if ["m0", "m1", "m2"].contains(&request.body().model.as_str()) {
            Err(DecisionError::Encode {
                message: "scripted encode failure".to_owned(),
            })
        } else {
            default_encode(request)
        }
    });
    let harness = Harness::new(Arc::new(backend), Arc::new(transport), config(8, 32), 5);
    let cancel = CancelToken::new();
    let mut stream = harness
        .runner
        .decide_stream(Box::new(requests(20).into_iter()), &cancel);
    let first: Vec<_> = stream.by_ref().take(3).collect();
    assert_eq!(first.len(), 3);
    assert!(first.iter().all(|result| result.position < 3));
    blocking.wait_started(8);
    drop(stream);
    assert!(
        !cancel.is_cancelled(),
        "abandon never cancels the caller's token"
    );
    blocking.release();
    let log = harness.finish(&dropped);
    let rows = rows_of(&log);
    let by_session = rows_by_session(&rows);
    assert_eq!(by_session.len(), rows.len(), "no duplicate session_id");
    let positions: BTreeSet<u64> = rows
        .iter()
        .map(|row| row.fields["position"].as_u64().expect("position"))
        .collect();
    assert_eq!(positions.len(), rows.len(), "one row per position");
    // The feeder submits in order: the rows are one per submitted job, positions 0..n.
    assert_eq!(positions, (0..rows.len() as u64).collect());
    assert!(
        rows.len() >= 11,
        "3 failed + 8 in flight at least, got {}",
        rows.len()
    );
    for row in &rows {
        let position = row.fields["position"].as_u64().expect("position");
        let id = row.fields["error"]["id"].as_str().expect("error id");
        let expected = if position < 3 {
            "decision.encode"
        } else {
            "decision.cancelled"
        };
        assert_eq!(id, expected, "position {position}");
    }
}

/// Result queue bound: workers 2, capacity 8, a consumer that reads nothing: transport
/// calls stop at 8 + 2; one read lets the next send through.
#[test]
fn slow_consumer_blocks_workers() {
    let (calls, on_call) = mpsc::channel();
    let calls = Mutex::new(calls);
    let (transport, _dropped) = FnTransport::new(move |request| {
        let _ = lock(&calls).send(index_of(request));
        Ok(ok())
    });
    let harness = Harness::new(
        Arc::new(ScriptedBackend::new()),
        Arc::new(transport),
        config(2, 8),
        5,
    );
    let mut stream = harness
        .runner
        .decide_stream(Box::new(requests(20).into_iter()), &CancelToken::new());
    for call in 0..10 {
        on_call
            .recv_timeout(FAILSAFE)
            .unwrap_or_else(|error| panic!("call {call} did not arrive: {error}"));
    }
    assert_eq!(
        on_call.recv_timeout(Duration::from_millis(500)).err(),
        Some(RecvTimeoutError::Timeout),
        "the 11th call waits for the consumer"
    );
    assert!(stream.next().is_some());
    assert!(
        on_call.recv_timeout(FAILSAFE).is_ok(),
        "one read lets the next send through"
    );
    let rest: Vec<_> = stream.collect();
    assert_eq!(rest.len(), 19);
}
