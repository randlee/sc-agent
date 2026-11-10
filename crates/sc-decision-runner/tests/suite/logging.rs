//! LogContext (RSH-009) and ANC-007 runner general records.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use sc_agent_http::HttpResponse;
use sc_agent_types::CancelToken;
use sc_decision_schema::Decision;

use crate::support::*;

/// LogContext: two concurrent requests busy once: their http.busy records carry their own
/// session_ids and positions.
#[test]
fn busy_records_carry_session() {
    let seen = Arc::new(Mutex::new(HashMap::<usize, usize>::new()));
    let counted = Arc::clone(&seen);
    let (transport, _dropped) = FnTransport::new(move |request| {
        let mut seen = lock(&counted);
        let calls = seen.entry(index_of(request)).or_default();
        *calls += 1;
        Ok(if *calls == 1 { busy("0") } else { ok() })
    });
    let harness = Harness::new(
        Arc::new(ScriptedBackend::new()),
        Arc::new(transport),
        config(2),
        5,
    );
    let results: Vec<_> = harness
        .runner
        .decide_stream(Box::new(requests(2).into_iter()), &CancelToken::new())
        .collect();
    assert!(results.iter().all(|result| result.outcome.is_ok()));
    let expected: BTreeSet<(String, u64)> = results
        .iter()
        .map(|result| (result.session_id.to_string(), result.position))
        .collect();
    let busy: BTreeSet<(String, u64)> = harness
        .records_of("sc_agent_http")
        .into_iter()
        .filter(|record| record.action == "http.busy")
        .map(|record| {
            (
                record.fields["session_id"]
                    .as_str()
                    .expect("session_id")
                    .to_owned(),
                record.fields["position"].as_u64().expect("position"),
            )
        })
        .collect();
    assert_eq!(busy, expected);
    assert_eq!(expected.len(), 2);
}

/// ANC-007: runner.stream.start, runner.stream.end, runner.worker.stopped and
/// runner.session_log_failed reach the GeneralLogger passed to DecisionRunner::new.
#[test]
fn runner_general_records() {
    let (transport, dropped) = FnTransport::new(|request| {
        Ok(match index_of(request) {
            1 => HttpResponse::new(500, vec![], b"down".to_vec()),
            2 => {
                let blob = "x".repeat(300 * 1024);
                HttpResponse::new(
                    200,
                    vec![],
                    format!(r#"{{"model":"m","answers":{{}},"usage":{{"input_tokens":1,"output_tokens":1}},"blob":"{blob}"}}"#)
                        .into_bytes(),
                )
            }
            _ => ok(),
        })
    });
    let harness = Harness::new(
        Arc::new(ScriptedBackend::new()),
        Arc::new(transport),
        config(2),
        5,
    );
    let results: Vec<_> = harness
        .runner
        .decide_stream(Box::new(requests(3).into_iter()), &CancelToken::new())
        .collect();
    assert_eq!(results.len(), 3);
    let failed_session = results
        .iter()
        .find(|result| result.position == 2)
        .map(|result| result.session_id.to_string())
        .expect("position 2");
    let log = harness.finish(&dropped);
    let records: Vec<_> = log
        .records()
        .expect("records")
        .into_iter()
        .filter(|record| record.target == "sc_decision_runner")
        .collect();
    let of = |action: &str| -> Vec<_> {
        records
            .iter()
            .filter(|record| record.action == action)
            .collect()
    };
    let start = of("runner.stream.start");
    assert_eq!(start.len(), 1);
    assert_eq!(start[0].fields["workers"], 2);
    assert_eq!(start[0].fields["queue_capacity"], 8);
    let run_id = start[0].fields["run_id"]
        .as_str()
        .expect("run_id")
        .to_owned();
    let end = of("runner.stream.end");
    assert_eq!(end.len(), 1);
    assert_eq!(end[0].fields["run_id"], run_id.as_str());
    assert_eq!(end[0].fields["results"], 3);
    assert_eq!(end[0].fields["failed"], 1);
    assert!(end[0].fields["live_threads"].is_u64());
    assert!(end[0].fields.get("abandoned").is_none());
    let stopped = of("runner.worker.stopped");
    assert_eq!(stopped.len(), 2, "one per worker");
    for record in &stopped {
        assert_eq!(record.fields["run_id"], run_id.as_str());
        assert_eq!(record.fields["reason"], "input_ended", "{record:?}");
    }
    let workers: BTreeSet<u64> = stopped
        .iter()
        .map(|record| record.fields["worker"].as_u64().expect("worker"))
        .collect();
    assert_eq!(workers, BTreeSet::from([0, 1]));
    let log_failed = of("runner.session_log_failed");
    assert_eq!(log_failed.len(), 1);
    assert_eq!(log_failed[0].fields["session_id"], failed_session);
    assert_eq!(log_failed[0].fields["position"], 2);
}

/// RSH-003: an abandoned run's runner.stream.end says so and counts the runner threads
/// still alive (here the 2 workers blocked in sends, plus at most the feeder).
#[test]
fn abandon_record_counts_live_threads() {
    let gate = Gate::default();
    let held = gate.clone();
    let (transport, dropped) = FnTransport::new(move |_| {
        held.pass();
        Ok(ok())
    });
    let harness = Harness::new(
        Arc::new(ScriptedBackend::new()),
        Arc::new(transport),
        config(2),
        5,
    );
    let stream = harness
        .runner
        .decide_stream(Box::new(requests(4).into_iter()), &CancelToken::new());
    gate.wait_inside(2);
    drop(stream);
    let end: Vec<_> = harness
        .records_of("sc_decision_runner")
        .into_iter()
        .filter(|record| record.action == "runner.stream.end")
        .collect();
    assert_eq!(end.len(), 1);
    assert_eq!(end[0].fields["abandoned"], true);
    let live = end[0].fields["live_threads"]
        .as_u64()
        .expect("live_threads");
    assert!(
        (2..=3).contains(&live),
        "2 blocked workers (+ the feeder): {live}"
    );
    gate.open();
    harness.finish(&dropped);
}
