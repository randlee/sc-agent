//! Check/judge contract tests. Names match the jev-check README parity rows.
//!
//! Transports and retry hooks come from `sc-agent-http` (`test-util`, via
//! the dev-dependency): `ScriptedTransport` answers in order and records
//! every request; `BusyPolicy::with_test_hooks` records waits instead of
//! sleeping.

use std::sync::Arc;
use std::time::Duration;

use sc_agent_http::test_util::{FakeClock, RecordingSleeper, ScriptedTransport};
use sc_agent_http::{BusyPolicy, Endpoint, HttpResponse};
use sc_agent_types::test_util::judge_outcome_field_names;
use sc_agent_types::{
    BackendId, BackendKeyId, CanonicalJudgePayload, JevError, JudgeBackend, JudgeRequest,
    LlmWireContext, NetworkErrorKind, PayloadBundle, ResolvedApiKey,
};
use sc_jev::{JevClient, JevClientConfig};
use serde_json::json;

const EXPECTED_BODY: &[u8] = b"{\"model\":\"jev-test\",\"questions\":[{\"choices\":[\"no\",\"yes\"],\"id\":\"q\",\"prompt\":\"x = 1\"}],\"state\":[]}";

const KEY: &str = "super-secret";

fn secret_key() -> ResolvedApiKey {
    ResolvedApiKey::try_new(BackendKeyId::TypeSafe, KEY.to_owned()).expect("key")
}

struct Harness {
    transport: ScriptedTransport,
    sleeper: RecordingSleeper,
    client: JevClient<ScriptedTransport>,
}

impl Harness {
    fn sent(&self) -> Vec<Vec<u8>> {
        self.transport
            .requests()
            .iter()
            .map(|request| request.body().to_vec())
            .collect()
    }

    fn waits(&self) -> Vec<Duration> {
        self.sleeper.waits()
    }
}

fn config() -> JevClientConfig {
    JevClientConfig::try_new(
        "jev-test",
        "jev.invalid",
        "/v1/x",
        3,
        Duration::from_secs(1),
    )
    .expect("config")
}

/// A client over `replies` (answered first to last) with zero jitter.
fn scripted_with(replies: Vec<Result<HttpResponse, JevError>>) -> Harness {
    let transport = ScriptedTransport::new(replies);
    let clock = FakeClock::new();
    let sleeper = RecordingSleeper::new(clock.clone());
    let policy = BusyPolicy::with_test_hooks(clock, sleeper.clone(), Duration::ZERO)
        .with_max_sends(config().attempts());
    let client =
        JevClient::with_config(config(), secret_key(), transport.clone()).with_busy_policy(policy);
    Harness {
        transport,
        sleeper,
        client,
    }
}

fn scripted(responses: Vec<HttpResponse>) -> Harness {
    scripted_with(responses.into_iter().map(Ok).collect())
}

/// Canonical payload with one `q` question whose instructions are `prompt`.
fn payload(prompt: &str) -> PayloadBundle {
    bundle(json!({
        "questions": [{"id": "q", "prompt": prompt, "choices": ["no", "yes"]}],
        "state": []
    }))
}

fn bundle(value: serde_json::Value) -> PayloadBundle {
    let payload: CanonicalJudgePayload = serde_json::from_value(value).expect("payload");
    PayloadBundle::encode(&payload).expect("bundle")
}

fn request(bundle: PayloadBundle, llm: Option<&str>) -> JudgeRequest {
    JudgeRequest::new(Arc::new(bundle), llm.map(LlmWireContext::new))
}

fn reply(body: serde_json::Value) -> HttpResponse {
    HttpResponse::new(200, Vec::new(), serde_json::to_vec(&body).expect("json"))
}

fn ok(model: &str) -> HttpResponse {
    reply(json!({
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
}

fn busy(status: u16, retry_after: Option<&str>) -> HttpResponse {
    let headers = retry_after
        .map(|value| vec![("Retry-After".to_owned(), value.to_owned())])
        .unwrap_or_default();
    HttpResponse::new(status, headers, Vec::new())
}

fn one_check_is_one_request_byte_exact() {
    let harness = scripted(vec![ok("jev-test")]);
    let outcome = harness
        .client
        .judge(&request(payload("x = 1"), None))
        .expect("judge");
    assert_eq!(harness.sent(), vec![EXPECTED_BODY.to_vec()]);
    let sent = &harness.sent()[0];
    let payload_bytes = payload("x = 1");
    assert_eq!(
        sent,
        &[b"{\"model\":\"jev-test\",", &payload_bytes.bytes()[1..]].concat()
    );
    assert_eq!(outcome.backend(), BackendId::Jev);
    let answer = &outcome.answers()["q"];
    assert_eq!(answer.choice().as_str(), "no");
    assert_eq!(answer.confidence(), Some(0.9));
    assert_eq!(answer.probabilities().expect("probabilities")["no"], 0.9);
    assert_eq!(
        outcome.provider_meta().expect("meta")["usage"]["input_tokens"],
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
    let requests = harness.transport.requests();
    let secret = requests[0].secret().expect("bearer");
    assert!(secret.matches(KEY));
    assert_eq!(secret.redacted(), "Authorization: Bearer [redacted]");
    assert_eq!(requests[0].endpoint().as_str(), "https://jev.invalid/v1/x");
    assert!(!format!("{:?}", harness.client).contains(KEY));
    let backend: &dyn JudgeBackend = &harness.client;
    assert_eq!(backend.id(), BackendId::Jev);
}

fn busy_is_retried_with_the_same_body() {
    let harness = scripted(vec![busy(529, Some("2")), ok("jev-test")]);
    harness
        .client
        .judge(&request(payload("x"), None))
        .expect("judge");
    let sent = harness.sent();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0], sent[1]);
    assert_eq!(harness.waits(), [Duration::from_secs(2)]);
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
            .judge(&request(payload("x = 1"), Some("DO NOT SEND THIS PROMPT")))
            .expect("judge");
        let sent = harness.sent();
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
        let req = request(payload(&"x".repeat(100_000)), None);
        let error = harness
            .client
            .validate_for_send(&req)
            .expect_err("validate");
        assert_eq!(error.code(), "INPUT_TOO_LARGE");
        let error = harness.client.judge(&req).expect_err("judge");
        assert_eq!(error.code(), "INPUT_TOO_LARGE");
        assert!(harness.sent().is_empty());
    }

    #[test]
    fn busy_statuses_retry_with_the_same_body() {
        for status in [429_u16, 502, 529] {
            let harness = scripted(vec![busy(status, None), ok("jev-test")]);
            harness
                .client
                .judge(&request(payload("x"), None))
                .expect("judge");
            let sent = harness.sent();
            assert_eq!(sent.len(), 2, "status {status}");
            assert_eq!(sent[0], sent[1]);
            assert_eq!(harness.waits(), [Duration::from_secs(1)], "status {status}");
        }
    }

    #[test]
    fn non_busy_error_is_not_retried() {
        let harness = scripted(vec![busy(500, Some("1")), ok("jev-test")]);
        let error = harness
            .client
            .judge(&request(payload("x"), None))
            .expect_err("status");
        assert_eq!(error.code(), "NETWORK_ERROR");
        assert_eq!(harness.sent().len(), 1);
        assert!(harness.waits().is_empty());
    }

    #[test]
    fn busy_attempts_exhausted() {
        let harness = scripted(vec![busy(429, None), busy(429, None), busy(429, None)]);
        let error = harness
            .client
            .judge(&request(payload("x"), None))
            .expect_err("exhausted");
        assert_eq!(error.code(), "NETWORK_ERROR");
        assert_eq!(
            error.to_string(),
            "network error: Jev HTTP 429 busy; 3 sends exhausted"
        );
        assert!(error.retryable());
        assert_eq!(harness.sent().len(), 3);
        // Absent Retry-After: exponential backoff (owner decision (b)).
        assert_eq!(
            harness.waits(),
            [Duration::from_secs(1), Duration::from_secs(2)]
        );
    }

    #[test]
    fn answer_ids_must_match_questions() {
        let harness = scripted(vec![reply(json!({"model": "jev-test", "answers": {}}))]);
        let error = harness
            .client
            .judge(&request(payload("x"), None))
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
        let harness = scripted(vec![reply(extra)]);
        let error = harness
            .client
            .judge(&request(payload("x"), None))
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
        let missing_case = scripted(vec![reply(missing)]);
        let error = missing_case
            .client
            .judge(&request(payload("x"), None))
            .expect_err("missing");
        assert_eq!(error.code(), "RESPONSE_INVALID");
    }

    #[test]
    fn wrong_model_and_non_json_are_response_invalid() {
        let harness = scripted(vec![ok("other-model")]);
        let error = harness
            .client
            .judge(&request(payload("x"), None))
            .expect_err("model");
        assert_eq!(error.code(), "RESPONSE_INVALID");

        let not_json = scripted(vec![HttpResponse::new(
            200,
            Vec::new(),
            b"not-json".to_vec(),
        )]);
        let error = not_json
            .client
            .judge(&request(payload("x"), None))
            .expect_err("json");
        assert_eq!(error.code(), "RESPONSE_INVALID");
    }

    #[test]
    fn non_canonical_bytes_never_become_a_bundle() {
        let canonical = payload("x");
        let mut spaced = canonical.bytes().to_vec();
        spaced.insert(1, b' ');
        let error = PayloadBundle::decode(&spaced).expect_err("whitespace");
        assert_eq!(error.code(), "INPUT_INVALID");
        assert!(error.to_string().contains("not canonical"));
        let reordered =
            br#"{"state":[],"questions":[{"choices":["no","yes"],"id":"q","prompt":"x"}]}"#;
        let error = PayloadBundle::decode(reordered).expect_err("key order");
        assert!(error.to_string().contains("not canonical"));
        assert_eq!(
            PayloadBundle::decode(canonical.bytes()).expect("canonical"),
            canonical
        );
    }

    #[test]
    fn invalid_question_sends_nothing() {
        let one_choice = bundle(json!({
            "questions": [{"id": "q", "prompt": "x", "choices": ["yes"]}],
            "state": []
        }));
        let harness = scripted(vec![ok("jev-test")]);
        let error = harness
            .client
            .judge(&request(one_choice, None))
            .expect_err("question");
        assert_eq!(error.code(), "INPUT_INVALID");
        assert!(harness.sent().is_empty());
    }

    #[test]
    fn array_questions_from_the_shared_contract_round_trip() {
        let bytes = bundle(json!({
            "questions": [{
                "choices": ["yes", "no"],
                "id": "relevant",
                "prompt": "Does it apply?"
            }],
            "state": []
        }));
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
        let harness = scripted(vec![HttpResponse::new(200, Vec::new(), body)]);
        let outcome = harness.client.judge(&request(bytes, None)).expect("judge");
        assert_eq!(outcome.answers()["relevant"].choice().as_str(), "yes");
        let sent = harness.sent();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].starts_with(br#"{"model":"jev-test","#));
    }

    #[test]
    fn blank_or_malformed_key_never_reaches_a_client() {
        let blank = ResolvedApiKey::try_new(BackendKeyId::TypeSafe, "  ".to_owned())
            .expect_err("blank key");
        assert_eq!(blank.code(), "KEY_MISSING");
        let spaced = ResolvedApiKey::try_new(BackendKeyId::TypeSafe, "super secret".to_owned())
            .expect_err("interior space");
        assert_eq!(spaced.code(), "CONFIG_INVALID");
        assert!(!spaced.fix_hint().contains("super secret"));
    }

    #[test]
    fn transport_errors_are_not_retried_and_drop_the_key() {
        let harness = scripted_with(vec![
            Err(JevError::network(
                NetworkErrorKind::Connect,
                "boom Bearer super-secret",
            )),
            Ok(ok("jev-test")),
        ]);
        let error = harness
            .client
            .judge(&request(payload("x"), None))
            .expect_err("network");
        assert_eq!(error.code(), "NETWORK_ERROR");
        assert!(
            error.retryable(),
            "connect failures are retryable by the caller"
        );
        assert_eq!(
            error.to_string(),
            "network error: Jev boom Bearer [redacted]"
        );
        assert!(!error.fix_hint().contains(KEY));
        assert_eq!(harness.sent().len(), 1);
        assert!(harness.waits().is_empty());
    }

    #[test]
    fn https_transport_refuses_cleartext_without_sending() {
        // The client only ever builds an https endpoint, and the shared
        // layer refuses plain HTTP in every build.
        let error = Endpoint::parse("http://api.typesafe.ai/v1/systemone").expect_err("cleartext");
        assert_eq!(error.code(), "CONFIG_INVALID");
        let packaged = JevClientConfig::packaged();
        assert!(packaged.endpoint().is_https());
        let client = JevClient::with_default_transport(secret_key());
        assert_eq!(client.config(), &packaged);
        assert!(!format!("{client:?}").contains(KEY));
    }

    #[test]
    fn packaged_attempts_match_the_shared_busy_policy() {
        assert_eq!(
            JevClientConfig::packaged().attempts(),
            BusyPolicy::default().max_sends()
        );
    }

    #[test]
    fn packaged_config_holds_model_host_path() {
        let packaged = JevClientConfig::packaged();
        assert_eq!(
            (
                packaged.model(),
                packaged.host(),
                packaged.path(),
                BackendId::Jev.descriptor().request_limit_bytes(),
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
        assert_eq!(
            packaged.endpoint().as_str(),
            "https://api.typesafe.ai/v1/systemone"
        );
    }
}
