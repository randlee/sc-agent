#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::time::Duration;

use sc_agent_http::test_server::{Server, ok};
use sc_agent_http::{Cancel, Policy};
use sc_decision_local::{Availability, Laya};
use sc_decision_schema::DecisionRequest;
use serde_json::json;

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
fn laya_reports_loaded_or_loads_on_demand() {
    assert_eq!(
        Laya.availability(br#"{"loaded":["m"]}"#, "m").unwrap(),
        Availability::Loaded
    );
    assert_eq!(
        Laya.availability(br#"{"loaded":null}"#, "m").unwrap(),
        Availability::LoadsOnDemand
    );
}

#[test]
fn local_runner_probes_once_then_decides_the_whole_batch() {
    let answer =
        |n: f64| ok(json!({ "model": "m", "answers": { "cat": { "type": "noul", "noul": n } } }));
    for health in [json!({"loaded": ["m"]}), json!({"loaded": null})] {
        let server = Server::start(vec![ok(health), answer(0.1), answer(0.2)]);
        let local =
            decidr::open_local(&server.base, "laya", policy(), None, Duration::from_secs(5))
                .unwrap();
        let mut got = Vec::new();
        local.decide_batch(
            vec![request(), request()],
            1,
            &Cancel::default(),
            &mut |o| got.push(o.result.is_ok()),
        );
        assert_eq!(got, vec![true, true]);
        let seen = server.finish();
        assert_eq!(
            seen.iter().map(|s| s.path.as_str()).collect::<Vec<_>>(),
            ["/health", "/v1/systemone", "/v1/systemone"]
        );
    }
}
