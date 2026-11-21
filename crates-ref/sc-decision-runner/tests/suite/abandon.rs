//! Abandon (RSH-006) and the result queue bound (RSH-011).

use std::collections::BTreeSet;
use std::sync::Arc;

use sc_agent_http::Transport;
use sc_agent_http::testing::BlockingTransport;
use sc_agent_types::CancelToken;
use sc_decision_schema::{Decision, DecisionError};

use crate::support::scripted_backend::default_encode;
use crate::support::*;

/// Abandon: 11 requests submitted (the gated input then blocks), 3 fail at encode, 8 sends
/// blocked; dropping the iterator after the 3 results stops intake, cancels the queue (not
/// the token), joins nothing, and writes exactly one row per started job: 11 rows, each
/// job's id as expected. Forced order: the input blocks after 11, so exactly 11 started.
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
    let harness = Harness::new(Arc::new(backend), Arc::new(transport), config(8), 5);
    let cancel = CancelToken::new();
    let (input, blocked, open) = GatedInput::new(requests(20), 11);
    let mut stream = harness.runner.decide_stream(Box::new(input), &cancel);
    let first: Vec<_> = stream.by_ref().take(3).collect();
    assert_eq!(first.len(), 3);
    assert!(first.iter().all(|result| result.position < 3));
    assert!(blocking.wait_started(8), "sends started");
    blocked
        .recv_timeout(HANG_GUARD)
        .expect("the feeder submitted 11 and waits for the 12th");
    drop(stream);
    assert!(
        !cancel.is_cancelled(),
        "abandon never cancels the caller's token"
    );
    // The dropping thread wrote the 8 in-flight orphans' rows before returning, while
    // the transport is still blocked.
    let orphans = rows_of(&harness.log)
        .into_iter()
        .filter(|row| row.fields["error"]["id"] == "decision.cancelled" && row.fields["sends"] == 1)
        .count();
    assert_eq!(
        orphans, 8,
        "one cancelled row per blocked send, before the release"
    );
    assert_eq!(blocking.sends(), 8, "still blocked");
    let _ = open.send(());
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
    assert_eq!(rows.len(), 11, "3 failed at encode + 8 cancelled in flight");
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

// The result-queue and job-queue capacity bounds (formerly `slow_consumer_blocks_workers` and
// `feeder_blocks_at_queue_capacity`, which proved blocking with a 500 ms timeout) are unit
// tests of `BoundedQueue` and `JobQueue` in src/queue/tests.rs, which read the number of
// waiting pushers and submitters under the lock (QA-R2-005).
