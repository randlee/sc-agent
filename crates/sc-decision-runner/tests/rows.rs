#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::time::Duration;

use sc_agent_http::test_server::Server;
use sc_agent_http::{Cancel, Policy};
use sc_agent_log::Session;
use sc_agent_types::Error;
use sc_decision_runner::{Decider, Decision};
use sc_decision_schema::DecisionRequest;
use sc_decision_systemone::SystemOne;
use serde_json::{Value, json};

fn policy() -> Policy {
    Policy {
        max_sends: 3,
        backoff: Duration::ZERO,
        timeout: Duration::from_secs(5),
    }
}

fn request() -> DecisionRequest {
    serde_json::from_value(json!({ "model": "m", "state": "a cat on a mat",
        "questions": { "cat": { "type": "noul" }, "color": { "type": "choice", "criteria": { "black": "", "white": "" } } } }))
    .unwrap()
}

#[test]
fn every_result_writes_a_session_row_with_the_request_text() {
    let root = std::env::temp_dir().join(format!("sc-decision-rows-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let session = Session::open(&root, "github.com/o/r".into(), "proj", "judge").unwrap();
    let server = Server::start(vec![(400, vec![], "no".into())]);
    let decider = Decider::new(
        SystemOne::new(&server.base, None).unwrap(),
        policy(),
        Some(session),
    );
    let outcome = decider.decide(request(), &Cancel::default());
    assert!(matches!(
        outcome.result,
        Err(Error::ApiStatus { status: 400, .. })
    ));
    drop(decider);
    server.finish();
    let file = root.join("sessions/github.com/o/r/judge/proj/sessions.jsonl");
    let text = std::fs::read_to_string(&file).unwrap();
    let row: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    let fields = &row["fields"];
    assert_eq!(fields["session_id"], outcome.session.to_string());
    assert_eq!(fields["error"]["id"], "api.status");
    assert_eq!(
        serde_json::from_str::<DecisionRequest>(fields["request"].as_str().unwrap()).unwrap(),
        request()
    );
    assert_eq!(fields["digests"]["cat"].as_str().unwrap().len(), 64);
    std::fs::remove_dir_all(root).unwrap();
}
