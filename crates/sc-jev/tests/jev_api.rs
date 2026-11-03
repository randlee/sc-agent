//! Check/judge contract tests. Names match the jev-check README parity rows.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use sc_agent_types::test_util::judge_outcome_field_names;
use sc_agent_types::{
    BackendId, JevError, JudgeBackend, JudgeRequest, LlmWireContext, PayloadBundle, ResolvedApiKey,
    canonical_json_bytes,
};
use sc_jev::{
    HttpsTransport, JevClient, JevClientConfig, JevTransport, RetryDelay, WireRequest, WireResponse,
};
use serde_json::json;

const EXPECTED_BODY: &[u8] = b"{\"model\":\"jev-test\",\"questions\":{\"q\":{\"criteria\":{\"no\":\"n\",\"yes\":\"y\"},\"instructions\":\"Is it?\",\"type\":\"choice\"}},\"state\":{\"code\":\"x = 1\"}}";

struct FakeTransport {
    responses: Mutex<Vec<WireResponse>>,
    sent: Mutex<Vec<Vec<u8>>>,
    authorizations: Mutex<Vec<String>>,
    fail_with_key: bool,
}

impl FakeTransport {
    fn new(responses: Vec<WireResponse>) -> Self {
        Self {
            responses: Mutex::new(responses),
            sent: Mutex::new(Vec::new()),
            authorizations: Mutex::new(Vec::new()),
            fail_with_key: false,
        }
    }

    fn failing(detail_includes_key: bool) -> Self {
        let mut transport = Self::new(Vec::new());
        transport.fail_with_key = detail_includes_key;
        transport
    }

    fn sent(&self) -> Vec<Vec<u8>> {
        self.sent.lock().expect("sent").clone()
    }

    fn authorizations(&self) -> Vec<String> {
        self.authorizations.lock().expect("auth").clone()
    }
}

impl JevTransport for FakeTransport {
    fn send(&self, request: &WireRequest<'_>) -> Result<WireResponse, JevError> {
        self.sent.lock().expect("sent").push(request.body.to_vec());
        let authorization = format!("Bearer {}", request.api_key.as_str());
        self.authorizations
            .lock()
            .expect("auth")
            .push(authorization.clone());
        if self.fail_with_key {
            return Err(JevError::NetworkError {
                detail: format!("boom {authorization}"),
            });
        }
        self.responses
            .lock()
            .expect("responses")
            .pop()
            .ok_or_else(|| JevError::NetworkError {
                detail: "fake transport has no response".to_owned(),
            })
    }
}

struct RecordingDelay {
    slept: Mutex<Vec<f64>>,
}

impl RecordingDelay {
    fn slept(&self) -> Vec<f64> {
        self.slept.lock().expect("slept").clone()
    }
}

impl RetryDelay for RecordingDelay {
    fn wait(&self, delay: Duration) {
        self.slept.lock().expect("slept").push(delay.as_secs_f64());
    }
}

struct Harness {
    transport: Arc<FakeTransport>,
    delay: Arc<RecordingDelay>,
    client: JevClient,
}

fn config() -> JevClientConfig {
    JevClientConfig::try_new(
        "jev-test",
        "jev.invalid",
        "/v1/x",
        400,
        3,
        Duration::from_secs(1),
    )
    .expect("config")
}

fn scripted(responses: Vec<WireResponse>) -> Harness {
    let transport = Arc::new(FakeTransport::new(responses));
    let delay = Arc::new(RecordingDelay {
        slept: Mutex::new(Vec::new()),
    });
    let client = JevClient::with_config(
        config(),
        ResolvedApiKey::new("super-secret".to_owned()),
        Arc::clone(&transport) as Arc<dyn JevTransport>,
    )
    .expect("client")
    .with_retry_delay(Arc::clone(&delay) as Arc<dyn RetryDelay>);
    Harness {
        transport,
        delay,
        client,
    }
}

fn questions() -> serde_json::Value {
    json!({
        "q": {
            "type": "choice",
            "instructions": "Is it?",
            "criteria": {"yes": "y", "no": "n"}
        }
    })
}

fn payload(state: serde_json::Value) -> Vec<u8> {
    canonical_json_bytes(&json!({
        "questions": questions(),
        "state": state,
    }))
    .expect("canonical")
}

fn request(bytes: Vec<u8>, llm: Option<&str>) -> JudgeRequest {
    JudgeRequest {
        payload: PayloadBundle::from_canonical_bytes(bytes),
        llm: llm.map(|system_prompt| LlmWireContext {
            system_prompt: system_prompt.to_owned(),
        }),
    }
}

fn ok(model: &str) -> WireResponse {
    WireResponse {
        status: 200,
        body: serde_json::to_vec(&json!({
            "model": model,
            "answers": {
                "q": {
                    "choice": "no",
                    "confidence": 0.9,
                    "probabilities": {"yes": 0.1, "no": 0.9}
                }
            },
            "usage": {"input_tokens": 318, "output_tokens": 31}
        }))
        .expect("json"),
        retry_after: None,
    }
}

fn busy(status: u16, retry_after: Option<&str>) -> WireResponse {
    WireResponse {
        status,
        body: Vec::new(),
        retry_after: retry_after.map(str::to_owned),
    }
}

fn one_check_is_one_request_byte_exact() {
    let harness = scripted(vec![ok("jev-test")]);
    let outcome = harness
        .client
        .judge(&request(payload(json!({"code": "x = 1"})), None))
        .expect("judge");
    assert_eq!(harness.transport.sent(), vec![EXPECTED_BODY.to_vec()]);
    let sent = &harness.transport.sent()[0];
    let payload_bytes = payload(json!({"code": "x = 1"}));
    assert_eq!(
        sent,
        &[b"{\"model\":\"jev-test\",", &payload_bytes[1..]].concat()
    );
    assert_eq!(outcome.backend, BackendId::Jev);
    assert_eq!(outcome.answers["q"]["choice"], "no");
    assert_eq!(outcome.answers["q"]["confidence"], 0.9);
    assert_eq!(
        outcome.probabilities.as_ref().expect("probabilities")["q"]["no"],
        0.9
    );
    assert_eq!(
        outcome.provider_meta.as_ref().expect("meta")["usage"]["input_tokens"],
        318
    );
    let value = serde_json::to_value(&outcome).expect("serialize");
    let names: Vec<&str> = value
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(names, judge_outcome_field_names());
    assert_eq!(
        harness.transport.authorizations(),
        vec!["Bearer super-secret".to_owned()]
    );
    assert!(!format!("{:?}", harness.client).contains("super-secret"));
    let backend: &dyn JudgeBackend = &harness.client;
    assert_eq!(backend.id(), BackendId::Jev);
}

fn busy_is_retried_with_the_same_body() {
    let harness = scripted(vec![ok("jev-test"), busy(529, Some("2"))]);
    harness
        .client
        .judge(&request(payload(json!({"code": "x"})), None))
        .expect("judge");
    let sent = harness.transport.sent();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0], sent[1]);
    assert_eq!(harness.delay.slept(), vec![2.0]);
}

#[test]
fn test_one_check_is_one_request_byte_exact() {
    one_check_is_one_request_byte_exact();
}

#[test]
fn test_busy_is_retried_with_the_same_body() {
    busy_is_retried_with_the_same_body();
}

mod jev_api {
    use super::*;

    #[test]
    fn test_one_check_is_one_request_byte_exact() {
        one_check_is_one_request_byte_exact();
    }

    #[test]
    fn test_busy_is_retried_with_the_same_body() {
        busy_is_retried_with_the_same_body();
    }

    #[test]
    fn llm_system_prompt_is_not_part_of_the_body() {
        let harness = scripted(vec![ok("jev-test")]);
        harness
            .client
            .judge(&request(
                payload(json!({"code": "x = 1"})),
                Some("DO NOT SEND THIS PROMPT"),
            ))
            .expect("judge");
        let sent = harness.transport.sent();
        assert_eq!(sent.len(), 1);
        assert!(
            !sent[0]
                .windows(b"DO NOT SEND THIS PROMPT".len())
                .any(|window| window == b"DO NOT SEND THIS PROMPT")
        );
        assert!(
            !sent[0]
                .windows(b"super-secret".len())
                .any(|window| window == b"super-secret")
        );
    }

    #[test]
    fn oversized_input_is_refused_and_nothing_is_sent() {
        let harness = scripted(vec![ok("jev-test")]);
        let req = request(payload(json!({"code": "x".repeat(500)})), None);
        let error = harness
            .client
            .validate_for_send(&req)
            .expect_err("validate");
        assert_eq!(error.code(), "INPUT_TOO_LARGE");
        let error = harness.client.judge(&req).expect_err("judge");
        assert_eq!(error.code(), "INPUT_TOO_LARGE");
        assert!(harness.transport.sent().is_empty());
    }

    #[test]
    fn busy_statuses_retry_with_the_same_body() {
        for status in [429_u16, 502, 529] {
            let harness = scripted(vec![ok("jev-test"), busy(status, None)]);
            harness
                .client
                .judge(&request(payload(json!({"code": "x"})), None))
                .expect("judge");
            let sent = harness.transport.sent();
            assert_eq!(sent.len(), 2, "status {status}");
            assert_eq!(sent[0], sent[1]);
            assert_eq!(harness.delay.slept(), vec![1.0]);
        }
    }

    #[test]
    fn non_busy_error_is_not_retried() {
        let harness = scripted(vec![ok("jev-test"), busy(500, Some("1"))]);
        let error = harness
            .client
            .judge(&request(payload(json!({"code": "x"})), None))
            .expect_err("status");
        assert_eq!(error.code(), "NETWORK_ERROR");
        assert_eq!(harness.transport.sent().len(), 1);
        assert!(harness.delay.slept().is_empty());
    }

    #[test]
    fn busy_attempts_exhausted() {
        let harness = scripted(vec![busy(429, None), busy(429, None), busy(429, None)]);
        let error = harness
            .client
            .judge(&request(payload(json!({"code": "x"})), None))
            .expect_err("exhausted");
        assert_eq!(error.code(), "NETWORK_ERROR");
        assert_eq!(harness.transport.sent().len(), 3);
        assert_eq!(harness.delay.slept().len(), 2);
    }

    #[test]
    fn answer_ids_must_match_questions() {
        let harness = scripted(vec![WireResponse {
            status: 200,
            body: serde_json::to_vec(&json!({"model": "jev-test", "answers": {}})).expect("json"),
            retry_after: None,
        }]);
        let error = harness
            .client
            .judge(&request(payload(json!({"code": "x"})), None))
            .expect_err("ids");
        assert_eq!(error.code(), "RESPONSE_INVALID");
    }

    #[test]
    fn probabilities_must_be_exactly_the_criteria() {
        let extra = json!({
            "model": "jev-test",
            "answers": {
                "q": {
                    "choice": "no",
                    "confidence": 0.9,
                    "probabilities": {"yes": 0.1, "no": 0.9, "maybe": 0.0}
                }
            }
        });
        let harness = scripted(vec![WireResponse {
            status: 200,
            body: serde_json::to_vec(&extra).expect("json"),
            retry_after: None,
        }]);
        let error = harness
            .client
            .judge(&request(payload(json!({"code": "x"})), None))
            .expect_err("extra");
        assert_eq!(error.code(), "RESPONSE_INVALID");

        let missing = json!({
            "model": "jev-test",
            "answers": {
                "q": {
                    "choice": "no",
                    "confidence": 0.9,
                    "probabilities": {"no": 0.9}
                }
            }
        });
        let missing_case = scripted(vec![WireResponse {
            status: 200,
            body: serde_json::to_vec(&missing).expect("json"),
            retry_after: None,
        }]);
        let error = missing_case
            .client
            .judge(&request(payload(json!({"code": "x"})), None))
            .expect_err("missing");
        assert_eq!(error.code(), "RESPONSE_INVALID");
    }

    #[test]
    fn wrong_model_and_non_json_are_response_invalid() {
        let harness = scripted(vec![ok("other-model")]);
        let error = harness
            .client
            .judge(&request(payload(json!({"code": "x"})), None))
            .expect_err("model");
        assert_eq!(error.code(), "RESPONSE_INVALID");

        let not_json = scripted(vec![WireResponse {
            status: 200,
            body: b"not-json".to_vec(),
            retry_after: None,
        }]);
        let error = not_json
            .client
            .judge(&request(payload(json!({"code": "x"})), None))
            .expect_err("json");
        assert_eq!(error.code(), "RESPONSE_INVALID");
    }

    #[test]
    fn invalid_question_sends_nothing() {
        let bytes = canonical_json_bytes(&json!({
            "questions": {
                "q": {"type": "text", "instructions": "x", "criteria": {"yes": "y"}}
            },
            "state": {}
        }))
        .expect("canonical");
        let harness = scripted(vec![ok("jev-test")]);
        let error = harness
            .client
            .judge(&request(bytes, None))
            .expect_err("question");
        assert_eq!(error.code(), "INPUT_INVALID");
        assert!(harness.transport.sent().is_empty());
    }

    #[test]
    fn payload_bytes_are_spliced_without_recanonicalizing() {
        let bytes = br#"{"questions": {"q":{"criteria":{"no":"n","yes":"y"},"instructions":"Is it?","type":"choice"}},"state":{"code":"x"}}"#.to_vec();
        let parsed: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        let recanonical = canonical_json_bytes(&parsed).expect("canonical");
        assert_ne!(bytes, recanonical);

        let harness = scripted(vec![ok("jev-test")]);
        harness
            .client
            .judge(&request(bytes.clone(), None))
            .expect("judge");
        let sent = harness.transport.sent();
        assert_eq!(sent.len(), 1);
        let expected = [br#"{"model":"jev-test","#, &bytes[1..]].concat();
        assert_eq!(sent[0], expected);
        let recanonical_body = [br#"{"model":"jev-test","#, &recanonical[1..]].concat();
        assert_ne!(sent[0], recanonical_body);
    }

    #[test]
    fn key_sorting_before_model_sends_nothing() {
        let bytes = canonical_json_bytes(&json!({
            "aaa": 1,
            "questions": questions(),
            "state": {}
        }))
        .expect("canonical");
        let harness = scripted(vec![ok("jev-test")]);
        let error = harness
            .client
            .judge(&request(bytes, None))
            .expect_err("key");
        assert_eq!(error.code(), "INPUT_INVALID");
        assert!(harness.transport.sent().is_empty());
    }

    #[test]
    fn array_questions_from_the_shared_contract_round_trip() {
        let bytes = canonical_json_bytes(&json!({
            "questions": [{
                "choices": ["yes", "no"],
                "id": "relevant",
                "prompt": "Does it apply?"
            }],
            "state": []
        }))
        .expect("canonical");
        let body = serde_json::to_vec(&json!({
            "model": "jev-test",
            "answers": {
                "relevant": {
                    "choice": "yes",
                    "confidence": 0.5,
                    "probabilities": {"no": 0.5, "yes": 0.5}
                }
            }
        }))
        .expect("json");
        let harness = scripted(vec![WireResponse {
            status: 200,
            body,
            retry_after: None,
        }]);
        let outcome = harness.client.judge(&request(bytes, None)).expect("judge");
        assert_eq!(outcome.answers["relevant"]["choice"], "yes");
        let sent = harness.transport.sent();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].starts_with(br#"{"model":"jev-test","#));
    }

    #[test]
    fn empty_key_sends_nothing() {
        let transport = Arc::new(FakeTransport::new(vec![ok("jev-test")]));
        let error = JevClient::with_config(
            config(),
            ResolvedApiKey::new("  ".to_owned()),
            Arc::clone(&transport) as Arc<dyn JevTransport>,
        )
        .expect_err("key");
        assert_eq!(error.code(), "KEY_MISSING");
        assert!(transport.sent().is_empty());
    }

    #[test]
    fn transport_errors_are_not_retried_and_drop_the_key() {
        let transport = Arc::new(FakeTransport::failing(true));
        let delay = Arc::new(RecordingDelay {
            slept: Mutex::new(Vec::new()),
        });
        let client = JevClient::with_config(
            config(),
            ResolvedApiKey::new("super-secret".to_owned()),
            Arc::clone(&transport) as Arc<dyn JevTransport>,
        )
        .expect("client")
        .with_retry_delay(delay);
        let error = client
            .judge(&request(payload(json!({"code": "x"})), None))
            .expect_err("network");
        assert_eq!(error.code(), "NETWORK_ERROR");
        assert!(!error.fix_hint().contains("super-secret"));
        assert_eq!(transport.sent().len(), 1);
    }

    #[test]
    fn https_transport_refuses_cleartext_without_sending() {
        let transport = HttpsTransport::new();
        let api_key = ResolvedApiKey::new("super-secret".to_owned());
        let error = transport
            .send(&WireRequest {
                url: "http://api.typesafe.ai/v1/systemone",
                body: b"{}",
                api_key: &api_key,
                timeout: Duration::from_secs(1),
            })
            .expect_err("cleartext");
        assert_eq!(error.code(), "NETWORK_ERROR");
        assert!(!error.fix_hint().contains("super-secret"));
    }

    #[test]
    fn packaged_config_holds_model_host_path() {
        let packaged = JevClientConfig::packaged();
        assert_eq!(
            (
                packaged.model(),
                packaged.host(),
                packaged.path(),
                packaged.max_request_bytes(),
                packaged.attempts(),
                packaged.timeout()
            ),
            (
                "jev-1.13.0",
                "api.typesafe.ai",
                "/v1/systemone",
                96_000,
                4,
                Duration::from_secs(60)
            )
        );
        assert_eq!(packaged.endpoint(), "https://api.typesafe.ai/v1/systemone");
    }
}
