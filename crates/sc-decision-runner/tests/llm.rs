#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::time::Duration;

use sc_agent_http::test_server::{Server, ok};
use sc_agent_http::{Cancel, Policy};
use sc_decision_llm::FromLlm;
use sc_decision_runner::{Decider, Decision};
use sc_decision_schema::{Answer, DecisionRequest};
use sc_llm_provider::Messages;
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

fn haiku_says(answers: Value) -> sc_agent_http::test_server::Reply {
    ok(
        json!({ "content": [{ "type": "text", "text": json!({ "answers": answers }).to_string() }],
        "stop_reason": "end_turn", "usage": { "input_tokens": 3, "output_tokens": 4 } }),
    )
}

#[test]
fn chat_model_answers_are_typed_and_a_wrong_type_is_resent() {
    let good = json!({ "cat": { "type": "noul", "noul": 0.9 },
        "color": { "type": "choice", "choice": "black", "confidence": 0.8, "probabilities": { "black": 0.8, "white": 0.2 } } });
    let server = Server::start(vec![
        haiku_says(
            json!({ "cat": { "type": "score", "score": 1, "confidence": 1, "probabilities": {} } }),
        ),
        haiku_says(good),
    ]);
    let decider = Decider::new(
        FromLlm::new(Messages::new(server.base.clone(), Some("k".into()))),
        policy(),
        None,
    );
    let outcome = decider.decide(request(), &Cancel::default());
    let response = outcome.result.unwrap();
    assert_eq!(response.answers["cat"], Answer::Noul { noul: 0.9 });
    assert!(
        matches!(&response.answers["color"], Answer::Choice { choice, .. } if choice == "black")
    );
    let seen = server.finish();
    assert_eq!(seen.len(), 2, "the mismatched answer was resent once");
    let schema = &seen[0].json()["output_config"]["format"]["schema"]["properties"]["answers"];
    assert_eq!(schema["required"], json!(["cat", "color"]));
    assert_eq!(
        schema["properties"]["color"]["properties"]["choice"]["enum"],
        json!(["black", "white"])
    );
}
