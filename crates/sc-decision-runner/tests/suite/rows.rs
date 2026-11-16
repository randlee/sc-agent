//! NFR-SAG-0001 / ADR-SAG-0002: one row per result; REQ-SAG-0202: digests with the response.

use std::collections::BTreeSet;
use std::sync::Arc;

use sc_agent_http::HttpResponse;
use sc_agent_types::CancelToken;
use sc_decision_schema::{Decision, DecisionError};

use crate::support::*;

/// NFR-SAG-0001 / ADR-SAG-0002: 50 requests (ok, failed, cancelled queued, cancelled in
/// flight): exactly one row per result, with its session ULID and identity fields; error
/// rows carry the DecisionError chain; "sends" is Attempts::sends().
#[test]
fn one_row_per_result() {
    let gate = Gate::default();
    let held = gate.clone();
    let (transport, dropped) = FnTransport::new(move |request| match index_of(request) {
        0..10 => Ok(ok()),
        10..15 => Ok(HttpResponse::new(409, vec![], b"down".to_vec())),
        _ => {
            held.pass();
            Ok(ok())
        }
    });
    let harness = Harness::of(Arc::new(transport));
    let cancel = CancelToken::new();
    let results = harness.stream(requests(50), &cancel);
    let mut all = take(&results, 15);
    gate.wait_inside(8);
    cancel.cancel();
    all.extend(take(&results, 35));
    gate.open();
    assert!(drain(&results).is_empty());
    let log = harness.finish(&dropped);
    let rows = rows_of(&log);
    assert_eq!(rows.len(), 50, "one row per result");
    let by_session = rows_by_session(&rows);
    assert_eq!(by_session.len(), 50);
    let positions: BTreeSet<u64> = all.iter().map(|result| result.position).collect();
    assert_eq!(positions, (0..50).collect());
    let (mut ok_rows, mut failed, mut cancelled, mut in_flight) = (0, 0, 0, 0);
    for result in &all {
        let row = &by_session[&result.session_id.to_string()][0].fields;
        assert_eq!(row["position"], result.position);
        for key in ["session_id", "repo", "project", "agent_role"] {
            assert!(row.get(key).is_some_and(|value| value.is_string()), "{key}");
        }
        assert_eq!(row["repo"], "local/test-host/sc-agent");
        assert_eq!(row["project"], "phase-a");
        assert_eq!(row["agent_role"], "runner-test");
        match &result.outcome {
            Ok(_) => {
                ok_rows += 1;
                assert_eq!(row["outcome"], "ok");
                assert_eq!(row["sends"], 1);
                assert!(row["response"].is_object());
            }
            Err(DecisionError::Cancelled { sends }) => {
                cancelled += 1;
                in_flight += usize::from(*sends == 1);
                assert_eq!(row["error"]["id"], "decision.cancelled");
                assert_eq!(row["sends"], *sends);
            }
            Err(_) => {
                failed += 1;
                assert_eq!(row["outcome"], "error");
                assert_eq!(row["error"]["id"], "decision.response_parse");
                assert!(
                    row["error"]["message"]
                        .as_str()
                        .is_some_and(|text| text.contains("409"))
                );
            }
        }
    }
    assert_eq!((ok_rows, failed, cancelled, in_flight), (10, 5, 35, 8));
}

/// REQ-SAG-0202: an ok row records one sc-sha digest per question (three -> three).
#[test]
fn three_questions_three_digests() {
    let (transport, _dropped) = FnTransport::new(|_| Ok(ok()));
    let harness = Harness::of(Arc::new(transport));
    let request = request_with("m0", &["a", "b", "c"]);
    let expected: Vec<(String, String)> = request
        .digests()
        .iter()
        .map(|(question, digest)| (question.clone(), digest.as_str().to_owned()))
        .collect();
    let result = harness.runner.decide(request, &CancelToken::new());
    assert!(result.outcome.is_ok(), "{result:?}");
    assert_eq!(result.digests.len(), 3);
    let rows = harness.rows();
    let digests = rows[0].fields["digests"].as_object().expect("digests");
    let recorded: Vec<(String, String)> = digests
        .iter()
        .map(|(question, digest)| (question.clone(), digest.as_str().expect("hex").to_owned()))
        .collect();
    assert_eq!(recorded, expected);
    assert_eq!(rows[0].fields["model"], "m0");
}
