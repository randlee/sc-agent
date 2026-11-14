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
            1 => HttpResponse::new(409, vec![], b"down".to_vec()),
            2 => {
                let blob = "x".repeat(sc_agent_log::MAX_ROW_BYTES + 1);
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
    assert_eq!(
        end[0].fields["live_threads"], 0,
        "every runner thread is uncounted before the last producer closes the results"
    );
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

/// RSH-003, QA-R2-006: an abandoned run's runner.stream.end says so and counts the runner
/// threads still alive: exactly 3, the 2 workers blocked in sends and the feeder blocked in
/// the gated input iterator.
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
    let (input, blocked, open) = GatedInput::new(requests(4), 2);
    let stream = harness
        .runner
        .decide_stream(Box::new(input), &CancelToken::new());
    gate.wait_inside(2);
    blocked
        .recv_timeout(HANG_GUARD)
        .expect("the feeder waits in the input iterator");
    drop(stream);
    let end: Vec<_> = harness
        .records_of("sc_decision_runner")
        .into_iter()
        .filter(|record| record.action == "runner.stream.end")
        .collect();
    assert_eq!(end.len(), 1);
    assert_eq!(end[0].fields["abandoned"], true);
    assert_eq!(
        end[0].fields["live_threads"], 3,
        "2 workers blocked in sends and the feeder blocked in the input"
    );
    let _ = open.send(());
    gate.open();
    harness.finish(&dropped);
}

/// A fixed timestamp and ManualClock keep every serialized envelope component stable.
fn decide_session_blob(
    bytes: usize,
) -> (
    sc_decision_schema::DecisionResult,
    sc_agent_log::testing::MemoryLog,
    serde_json::Value,
) {
    use sc_agent_http::testing::{ManualClock, ScriptedTransport};
    use sc_agent_http::{HttpClient, ResendPolicy};
    use sc_agent_log::testing::MemoryLog;
    use sc_agent_log::{Repo, SessionIdentity, SessionLogger};
    use sc_decision_runner::{DecisionRunner, RunnerConfig};
    use sc_decision_schema::SystemOneResponse;

    let log = MemoryLog::new().expect("log");
    let body = serde_json::json!({"model":"m", "answers":{"q":{"type":"noul","noul":0.5}}, "usage":{"input_tokens":1,"output_tokens":1}, "blob":"x".repeat(bytes)}).to_string();
    let expected = serde_json::to_value(
        SystemOneResponse::from_json(200, body.as_bytes()).expect("expected response"),
    )
    .expect("response JSON");
    let transport =
        ScriptedTransport::responses(vec![HttpResponse::new(200, vec![], body.into_bytes())]);
    let http = HttpClient::new(Arc::new(transport), ResendPolicy::default(), log.general());
    let identity =
        SessionIdentity::new(Repo::local("host", "folder").expect("repo"), "b-1", "test")
            .expect("identity");
    let session = SessionLogger::injected_with_timestamp(
        log.logger(),
        identity,
        sc_observability::Timestamp::UNIX_EPOCH,
    );
    let runner = DecisionRunner::with_clock(
        Arc::new(ScriptedBackend::new()),
        http,
        session,
        log.general(),
        RunnerConfig::new(1).expect("worker"),
        ManualClock::new().clock(),
    );
    (
        runner.decide(request("m"), &CancelToken::new()),
        log,
        expected,
    )
}

fn session_envelope_bytes() -> usize {
    let (_, log, _) = decide_session_blob(0);
    let events = log.events().expect("flush calibration");
    let event = events
        .iter()
        .find(|event| event.target.as_str() == sc_agent_log::SessionLogger::TARGET)
        .expect("calibration session row");
    serde_json::to_vec(event)
        .expect("serialize calibration")
        .len()
}

/// REQ-QA-001: exactly MAX_ROW_BYTES is written without runner.session_log_failed.
#[test]
fn session_row_at_max_row_bytes_is_written_without_failure() {
    use sc_agent_log::{MAX_ROW_BYTES, SessionLogger};
    let (result, log, expected) = decide_session_blob(MAX_ROW_BYTES - session_envelope_bytes());
    assert_eq!(
        serde_json::to_value(result.outcome.expect("decision preserved")).expect("response JSON"),
        expected
    );
    let events = log.events().expect("flush session row");
    let rows: Vec<_> = events
        .iter()
        .filter(|event| event.target.as_str() == SessionLogger::TARGET)
        .collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        serde_json::to_vec(rows[0]).expect("serialize row").len(),
        MAX_ROW_BYTES
    );
    assert!(
        log.records()
            .expect("flush general records")
            .iter()
            .all(|event| event.action != "runner.session_log_failed")
    );
}

/// REQ-QA-001: one byte over MAX_ROW_BYTES preserves the decision and reports exact refusal.
#[test]
fn session_row_one_byte_over_max_row_bytes_preserves_decision() {
    use sc_agent_log::{LogError, LogKind, MAX_ROW_BYTES, SessionLogger};
    use sc_agent_types::ErrorId;
    let (result, log, expected) = decide_session_blob(MAX_ROW_BYTES - session_envelope_bytes() + 1);
    let session_id = result.session_id.to_string();
    assert_eq!(
        serde_json::to_value(result.outcome.expect("decision preserved")).expect("response JSON"),
        expected
    );
    let records = log.records().expect("flush refused row");
    assert!(
        records
            .iter()
            .all(|event| event.target != SessionLogger::TARGET)
    );
    let failures: Vec<_> = records
        .iter()
        .filter(|event| event.action == "runner.session_log_failed")
        .collect();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].fields["session_id"], session_id);
    assert_eq!(failures[0].fields["position"], 0);
    let error = LogError::RowTooLarge {
        log: LogKind::Session,
        bytes: MAX_ROW_BYTES + 1,
        max: MAX_ROW_BYTES,
    };
    assert_eq!(
        failures[0].fields["error"],
        serde_json::to_value(error.detail()).expect("typed refusal detail")
    );
}
