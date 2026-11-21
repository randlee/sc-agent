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
    for (health, expected) in [
        (ok(json!({"loaded": ["m"]})), true),
        (ok(json!({"loaded": null})), true),
        ((401, vec![], "probe refused".into()), false),
    ] {
        let mut replies = vec![health];
        if expected {
            replies.extend([answer(0.1), answer(0.2)]);
        }
        let server = Server::start(replies);
        let port: u16 = server.base.rsplit(':').next().unwrap().parse().unwrap();
        let config: sc_agent_config::Config = serde_json::from_value(json!({"local_runners": {"test": {"kind":"laya", "host":"127.0.0.1", "port":port, "request_timeout_secs":10}}})).unwrap();
        let root = tempfile::tempdir().unwrap();
        let logger = sc_agent_log::general("t", root.path()).unwrap();
        let mut got = Vec::new();
        decidr::send(
            vec![request(), request()],
            "m",
            None,
            Some("test"),
            &config,
            Some(1),
            None,
            None,
            None,
            None,
            &logger,
            &Cancel::default(),
            &mut |o| {
                got.push(o.result.is_ok());
                if !expected {
                    assert_eq!(o.result.as_ref().unwrap_err().id(), "api.status");
                }
            },
        )
        .unwrap();
        assert_eq!(got, vec![expected, expected]);
        let seen = server.finish();
        assert_eq!(
            seen.iter().map(|s| s.path.as_str()).collect::<Vec<_>>(),
            if expected {
                vec!["/health", "/v1/systemone", "/v1/systemone"]
            } else {
                vec!["/health"]
            }
        );
    }
}

#[test]
fn first_load_deadline_adds_load_time_to_the_request_override() {
    let normal = Policy {
        timeout: Duration::from_millis(10125),
        ..policy()
    };
    let load = Duration::from_secs(600);
    assert_eq!(
        decidr::load_policy(normal, load).timeout,
        normal.timeout + load
    );
    assert_eq!(normal.timeout, Duration::from_millis(10125));
}
