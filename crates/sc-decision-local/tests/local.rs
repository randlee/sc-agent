#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::time::Duration;

use sc_agent_http::test_server::{Server, ok};
use sc_agent_http::{Cancel, Policy};
use sc_decision_local::{Availability, Generic, Laya, Local, LocalDaemon, Mapika, Strands};
use sc_decision_runner::{Decider, Decision};
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
fn daemons_read_their_own_health() {
    assert_eq!(
        Laya.availability(br#"{"loaded":["m"]}"#, "m").unwrap(),
        Availability::Loaded
    );
    assert_eq!(
        Laya.availability(br#"{"loaded":null}"#, "m").unwrap(),
        Availability::LoadsOnDemand
    );
    assert_eq!(
        Strands.availability(br#"{"model":"x"}"#, "m").unwrap(),
        Availability::Other(vec!["x".into()])
    );
    assert_eq!(
        Mapika
            .availability(br#"{"ok":false,"model":"m"}"#, "m")
            .unwrap(),
        Availability::Loading
    );
    assert_eq!(
        Mapika
            .availability(br#"{"ok":true,"model":"m"}"#, "m")
            .unwrap(),
        Availability::Loaded
    );
    assert_eq!(
        Generic
            .availability(br#"{"models":[{"name":"m"}]}"#, "m")
            .unwrap(),
        Availability::Loaded
    );
}

#[test]
fn local_runner_probes_once_then_decides_the_whole_batch() {
    let answer =
        |n: f64| ok(json!({ "model": "m", "answers": { "cat": { "type": "noul", "noul": n } } }));
    let server = Server::start(vec![
        ok(json!({ "loaded": ["m"] })),
        answer(0.1),
        answer(0.2),
    ]);
    let local = Local::new(
        &server.base,
        "laya",
        |s| Decider::new(s, policy(), None),
        Duration::from_secs(5),
    )
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

#[test]
fn local_runner_serving_another_model_fails_every_item_without_sending() {
    let server = Server::start(vec![ok(json!({ "model": "other" }))]);
    let local = Local::new(
        &server.base,
        "strands",
        |s| Decider::new(s, policy(), None),
        Duration::from_secs(5),
    )
    .unwrap();
    let mut got = Vec::new();
    local.decide_batch(
        vec![request(), request()],
        2,
        &Cancel::default(),
        &mut |o| got.push(o.position),
    );
    assert_eq!(got, vec![0, 1]);
    assert_eq!(server.finish().len(), 1);
}
