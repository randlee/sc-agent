#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use sc_decision_schema::DecisionRequest;
use serde_json::json;

fn request() -> DecisionRequest {
    serde_json::from_value(json!({ "model": "m", "state": "a cat on a mat",
        "questions": { "cat": { "type": "noul" }, "color": { "type": "choice", "criteria": { "black": "", "white": "" } } } }))
    .unwrap()
}

#[test]
fn digest_is_stable_and_per_question() {
    let r = request();
    let a = DecisionRequest::digest("cat", &r.questions["cat"]).unwrap();
    assert_eq!(a, DecisionRequest::digest("cat", &r.questions["cat"]).unwrap());
    assert_ne!(a, DecisionRequest::digest("dog", &r.questions["cat"]).unwrap());
}
