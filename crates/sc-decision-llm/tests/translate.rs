//! DecisionRequest -> LLM call -> SystemOneResponse, over canned provider replies.

// Clippy allows unwrap, expect, panic and indexing only in test code (clippy.toml);
// the whole file, helpers included, is test code.
#![cfg(test)]

mod common;

use std::sync::Arc;

use common::{
    KEY, anthropic_reply, chat_reply, choice_request, four_answers, four_questions, haiku,
    json_response, luna,
};
use sc_agent_http::{HttpRequest, LogContext};
use sc_agent_types::{ErrorDetail, ErrorId};
use sc_decision_llm::LlmDecisionBackend;
use sc_decision_schema::{
    Answer, Backend, BoxCoordinates, DecisionError, DecisionRequest, ImageFormat, ImagePart,
    SystemOneRequest,
};
use serde_json::{Value, json};

/// The old answer fixture (sc-jev-backends tests/llm.rs), now one choice answer.
fn answer_text() -> String {
    json!({
        "answers": {
            "q": {
                "choice": "no",
                "confidence": 0.7,
                "probabilities": { "yes": 0.3, "no": 0.7 }
            }
        }
    })
    .to_string()
}

fn body(request: &HttpRequest) -> Value {
    serde_json::from_slice(request.body()).expect("json body")
}

/// The ids of a detail chain, top first.
fn chain(detail: &ErrorDetail) -> Vec<String> {
    let mut ids = vec![detail.id.clone()];
    let mut level = detail.cause.as_deref();
    while let Some(next) = level {
        ids.push(next.id.clone());
        level = next.cause.as_deref();
    }
    ids
}

fn mismatch(error: &DecisionError) -> &ErrorDetail {
    match error {
        DecisionError::SchemaMismatch { backend, detail } => {
            assert_eq!(*backend, "llm");
            detail
        }
        other => panic!("not a schema mismatch: {other:?}"),
    }
}

fn backend_failure(error: &DecisionError) -> &ErrorDetail {
    match error {
        DecisionError::Backend { backend, detail } => {
            assert_eq!(*backend, "llm");
            detail
        }
        other => panic!("not a backend failure: {other:?}"),
    }
}

/// AC 6: the schema requires all four ids and every per-type field, through output_config
/// (Haiku) and response_format strict (Luna).
#[test]
fn schema_requires_every_id_and_field() {
    let request = four_questions();
    let anthropic = body(&haiku().encode(&request).expect("encode"));
    let format = &anthropic["output_config"]["format"];
    assert_eq!(format["type"], "json_schema");
    let schema = &format["schema"];
    let chat = body(&luna().encode(&request).expect("encode"));
    assert_eq!(chat["response_format"]["type"], "json_schema");
    assert_eq!(chat["response_format"]["json_schema"]["strict"], true);
    assert_eq!(&chat["response_format"]["json_schema"]["schema"], schema);

    assert_eq!(schema["required"], json!(["answers"]));
    assert_eq!(schema["additionalProperties"], false);
    let answers = &schema["properties"]["answers"];
    assert_eq!(answers["additionalProperties"], false);
    assert_eq!(
        answers["required"],
        json!(["urgent", "team", "severity", "where"])
    );
    let per = &answers["properties"];
    assert_eq!(per["urgent"]["required"], json!(["noul"]));
    assert_eq!(per["urgent"]["properties"]["noul"]["type"], "number");
    assert_eq!(
        per["team"]["required"],
        json!(["choice", "confidence", "probabilities"])
    );
    assert_eq!(
        per["team"]["properties"]["choice"]["enum"],
        json!(["facilities", "it"])
    );
    let team_probs = &per["team"]["properties"]["probabilities"];
    assert_eq!(team_probs["required"], json!(["facilities", "it"]));
    assert_eq!(team_probs["additionalProperties"], false);
    assert_eq!(
        per["severity"]["required"],
        json!(["score", "confidence", "probabilities"])
    );
    assert_eq!(
        per["severity"]["properties"]["probabilities"]["required"],
        json!(["0", "1", "2"])
    );
    assert_eq!(per["where"]["required"], json!(["box", "confidence"]));
    assert_eq!(
        per["where"]["properties"]["box"]["required"],
        json!(["top", "bottom", "left", "right"])
    );
    assert_eq!(
        per["where"]["properties"]["confidence"]["type"],
        json!(["number", "null"])
    );
    for object in [
        &per["urgent"],
        &per["team"],
        &per["severity"],
        &per["where"],
    ] {
        assert_eq!(object["additionalProperties"], false);
    }

    // The built-in prompt, then the state and questions as compact JSON (REQ-SAG-0006: no
    // consumer prompt).
    assert!(
        anthropic["system"]
            .as_str()
            .is_some_and(|prompt| prompt.starts_with("You answer decision questions"))
    );
    assert_eq!(anthropic["model"], "claude-haiku-5-5");
    assert_eq!(anthropic["max_tokens"], 16_000);
    let text = anthropic["messages"][0]["content"][0]["text"]
        .as_str()
        .expect("text part");
    let sent: Value = serde_json::from_str(text).expect("json text");
    assert_eq!(
        sent,
        json!({
            "state": { "ticket": "printer on fire", "floor": 3 },
            "questions": request.body().questions
        })
    );
    assert!(text.starts_with(r#"{"state":{"ticket""#), "{text}");
    assert_eq!(chat["model"], "gpt-6-luna");
    assert_eq!(chat["max_completion_tokens"], 16_000);
    assert_eq!(
        body(
            &haiku()
                .with_max_tokens(900)
                .encode(&request)
                .expect("encode")
        )["max_tokens"],
        900
    );
}

/// AC 5, AC 12: a reply for all four parses as the Jev answer types, legend from the score
/// criteria, values unchanged (0.123456789 stays 0.123456789).
#[test]
fn four_answers_parse_as_jev_answers() {
    let request = four_questions();
    for (backend, reply, model, usage) in [
        (
            haiku(),
            anthropic_reply(&four_answers().to_string(), "end_turn"),
            "claude-haiku-5-5-20261007",
            (120, 40),
        ),
        (
            luna(),
            chat_reply(&four_answers().to_string(), "stop"),
            "gpt-6-luna-2026-09-22",
            (110, 35),
        ),
    ] {
        let response = backend
            .decode(reply, &request, &LogContext::NONE)
            .expect("decode");
        assert_eq!(response.model, model);
        assert_eq!(
            (response.usage.input_tokens, response.usage.output_tokens),
            usage
        );
        let ids: Vec<&str> = response.answers.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["urgent", "team", "severity", "where"]);
        let answers: Vec<&Answer> = response.answers.iter().map(|(_, a)| a).collect();
        assert_eq!(answers[0], &Answer::Noul { noul: 0.123456789 });
        let Answer::Choice {
            choice,
            confidence,
            probabilities,
        } = answers[1]
        else {
            panic!("choice: {:?}", answers[1]);
        };
        assert_eq!((choice.as_str(), *confidence), ("facilities", 0.8));
        let probs: Vec<(&str, f64)> = probabilities
            .iter()
            .map(|(key, value)| (key.as_str(), *value))
            .collect();
        assert_eq!(probs, [("facilities", 0.8), ("it", 0.2)]);
        let Answer::Score {
            score,
            confidence,
            legend,
            probabilities,
        } = answers[2]
        else {
            panic!("score: {:?}", answers[2]);
        };
        assert_eq!((*score, *confidence), (2.0, 0.7));
        let legend: Vec<(&str, &Value)> = legend.iter().map(|(k, v)| (k.as_str(), v)).collect();
        assert_eq!(
            legend,
            [
                ("0", &json!("low")),
                ("1", &json!("medium")),
                ("2", &json!("high"))
            ]
        );
        assert_eq!(probabilities.len(), 3);
        assert_eq!(
            answers[3],
            &Answer::Box {
                coordinates: BoxCoordinates {
                    top: 0.1,
                    bottom: 0.9,
                    left: 0.2,
                    right: 0.8
                },
                confidence: None
            }
        );
        // REQ-SAG-0007: the exact text comes back out.
        let wire = serde_json::to_string(&response).expect("encode");
        assert!(wire.contains(r#""noul":0.123456789"#), "{wire}");
    }
}

/// AC 7: replies that do not match the schema are decision.schema_mismatch with the cause id.
#[test]
fn mismatches_carry_the_cause() {
    let request = choice_request();
    let backend = haiku();
    let decode = |text: &str, stop: &str| {
        backend
            .decode(anthropic_reply(text, stop), &request, &LogContext::NONE)
            .expect_err("mismatch")
    };
    // The old fixture matches the schema.
    assert!(
        backend
            .decode(
                anthropic_reply(&answer_text(), "end_turn"),
                &request,
                &LogContext::NONE
            )
            .is_ok()
    );

    let no_probs = decode(
        &json!({ "answers": { "q": { "choice": "no", "confidence": 0.7 } } }).to_string(),
        "end_turn",
    );
    assert_eq!(no_probs.id(), "decision.schema_mismatch");
    let detail = mismatch(&no_probs);
    assert_eq!(chain(detail), ["decision_llm.answer_invalid"]);
    assert!(
        detail.message.contains("probabilities"),
        "{}",
        detail.message
    );

    let ids = decode(
        &json!({ "answers": { "other": { "choice": "no", "confidence": 0.7, "probabilities": {} } } })
            .to_string(),
        "end_turn",
    );
    let detail = mismatch(&ids);
    assert_eq!(chain(detail), ["decision_llm.answer_ids"]);
    assert!(detail.message.contains("missing: [q]") && detail.message.contains("extra: [other]"));

    let outside = decode(
        &json!({ "answers": { "q": { "choice": "maybe", "confidence": 0.7, "probabilities": { "yes": 0.5, "no": 0.5 } } } })
            .to_string(),
        "end_turn",
    );
    assert!(mismatch(&outside).message.contains("maybe"));

    let mistyped = decode(
        &json!({ "answers": { "q": { "choice": "no", "confidence": "high", "probabilities": { "yes": 0.3, "no": 0.7 } } } })
            .to_string(),
        "end_turn",
    );
    assert_eq!(chain(mismatch(&mistyped)), ["decision_llm.answer_invalid"]);

    let truncated = decode(&answer_text(), "max_tokens");
    assert_eq!(
        chain(mismatch(&truncated)),
        ["decision_llm.llm", "llm.truncated"]
    );

    let not_json = decode("Sure! The answer is no.", "end_turn");
    assert_eq!(
        chain(mismatch(&not_json)),
        ["decision_llm.llm", "llm.output_not_json"]
    );

    let no_answers = decode(r#"{"result": 1}"#, "end_turn");
    assert_eq!(chain(mismatch(&no_answers)), ["decision_llm.answer_ids"]);
}

/// AC 7: a refusal is decision.backend, not a mismatch (no resend signal).
#[test]
fn refusal_is_a_backend_failure() {
    let error = haiku()
        .decode(
            anthropic_reply("I won't answer that.", "refusal"),
            &choice_request(),
            &LogContext::NONE,
        )
        .expect_err("refused");
    assert_eq!(error.id(), "decision.backend");
    assert_eq!(
        chain(backend_failure(&error)),
        ["decision_llm.llm", "llm.refused"]
    );
}

/// The request with one inline asset and one multipart asset.
fn image_request() -> DecisionRequest {
    let body = SystemOneRequest::from_json(
        r#"{"model":"haiku","state":"compare the images","questions":{"q":{"type":"box"}},"assets":{"inline":{"type":"image","format":"png","data_base64":"iVBORw0KGgoAAAA="},"upload":{"type":"image","part":"img1"}}}"#,
    )
    .expect("body");
    let part = ImagePart {
        name: "img1".to_owned(),
        format: ImageFormat::Jpeg,
        bytes: vec![0xff, 0xd8, 0xff, 0xe0, 0x01],
    };
    DecisionRequest::new(body, vec![part], false).expect("valid by construction")
}

/// AC 8, AC 9: every asset is sent as an image part after the JSON text (inline data as
/// given, a multipart part's bytes base64-encoded); a provider 400 rejecting it is
/// decision.backend with cause llm.api_status and the provider's whole message.
#[test]
fn assets_are_passed_through_as_images() {
    let request = image_request();
    let anthropic = body(&haiku().encode(&request).expect("encode"));
    let content = &anthropic["messages"][0]["content"];
    assert_eq!(content.as_array().map(Vec::len), Some(5));
    assert_eq!(content[0]["type"], "text");
    assert_eq!(
        content[1],
        json!({ "type": "text", "text": "asset inline:" })
    );
    assert_eq!(
        content[2],
        json!({ "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": "iVBORw0KGgoAAAA=" } })
    );
    assert_eq!(
        content[3],
        json!({ "type": "text", "text": "asset upload:" })
    );
    assert_eq!(
        content[4],
        json!({ "type": "image", "source": { "type": "base64", "media_type": "image/jpeg", "data": "/9j/4AE=" } })
    );
    let chat = body(&luna().encode(&request).expect("encode"));
    assert_eq!(
        chat["messages"][1]["content"][4]["image_url"]["url"],
        "data:image/jpeg;base64,/9j/4AE="
    );

    let message = "messages.0.content.2.image.source: image exceeds the 5 MB limit for this model";
    let rejected = json_response(
        400,
        &json!({ "type": "error", "error": { "type": "invalid_request_error", "message": message } }),
    );
    let error = haiku()
        .decode(rejected, &request, &LogContext::NONE)
        .expect_err("rejected");
    assert_eq!(error.id(), "decision.backend");
    let detail = backend_failure(&error);
    assert_eq!(chain(detail), ["decision_llm.llm", "llm.api_status"]);
    let api = detail.cause.as_deref().expect("llm level");
    assert!(api.message.contains(message), "{}", api.message);
    assert!(!format!("{error:?}").contains(KEY));
}

/// AC 11: LlmDecisionBackend is a sc_decision_schema::Backend trait object.
#[test]
fn backend_is_a_trait_object() {
    let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(haiku()), Arc::new(luna())];
    let request = four_questions();
    for backend in &backends {
        let http = backend.encode(&request).expect("encode");
        assert!(http.secret().is_some());
        let rendered = format!("{backend:?} {http:?}");
        assert!(!rendered.contains(KEY), "{rendered}");
    }
    let haiku: LlmDecisionBackend = haiku();
    assert_eq!(haiku.model().as_str(), "claude-haiku-5-5");
    assert_eq!(LlmDecisionBackend::DEFAULT_MAX_TOKENS, 16_000);
}
