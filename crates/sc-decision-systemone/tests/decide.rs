#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::time::Duration;

use sc_agent_http::test_server::{Server, ok};
use sc_agent_http::{Cancel, Policy};
use sc_decision_runner::{Decider, Decision};
use sc_decision_schema::DecisionRequest;
use sc_decision_systemone::SystemOne;
use serde_json::json;

fn policy() -> Policy {
    Policy { max_sends: 3, backoff: Duration::ZERO, timeout: Duration::from_secs(5) }
}

fn request() -> DecisionRequest {
    serde_json::from_value(json!({ "model": "m", "state": "a cat on a mat",
        "questions": { "cat": { "type": "noul" }, "color": { "type": "choice", "criteria": { "black": "", "white": "" } } } }))
    .unwrap()
}

#[test]
fn jev_api_passes_the_request_through() {
    let reply = json!({ "model": "jev-1.14.0", "answers": { "cat": { "type": "noul", "noul": 0.1 } }, "usage": {}, "trace": "kept" });
    let server = Server::start(vec![ok(reply)]);
    let decider = Decider::new(SystemOne::new(&server.base, Some("k")).unwrap(), policy(), None);
    let response = decider.decide(request(), &Cancel::default()).result.unwrap();
    assert_eq!(response.extra["trace"], "kept");
    let seen = server.finish();
    assert_eq!(seen[0].path, "/v1/systemone");
    assert_eq!(seen[0].json(), serde_json::to_value(request()).unwrap());
}
