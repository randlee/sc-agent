//! Scripted-transport coverage for Haiku and Luna (sprint a-5; r-3 moved the
//! transport and retry doubles to `sc-agent-http` `test-util`).

use std::sync::Arc;
use std::time::Duration;

use sc_agent_http::test_util::{FakeClock, RecordingSleeper, ScriptedTransport};
use sc_agent_http::{BusyPolicy, HttpRequest, HttpResponse, Timeouts, UreqTransport};
use sc_agent_types::test_util::judge_outcome_field_names;
use sc_agent_types::{
    BackendId, BackendKeyId, CanonicalJudgePayload, JudgeBackend, JudgeOutcome, JudgeRequest,
    LlmWireContext, PayloadBundle, ResolvedApiKey, Sha256Digest,
};
use sc_jev_backends::{AnthropicHaikuBackend, OpenAiLunaBackend};
use serde_json::{Value, json};

const PROMPT: &str = "INJECTED-EVALUATOR-PROMPT";
const KEY: &str = "test-api-key";

/// The transport and sleeper a test inspects after `judge`.
struct Recorded {
    transport: ScriptedTransport,
    sleeper: RecordingSleeper,
}

impl Recorded {
    fn delays(&self) -> Vec<Duration> {
        self.sleeper.waits()
    }
}

/// Replies are listed last-first: the last entry answers the first send.
fn script(mut replies: Vec<HttpResponse>) -> ScriptedTransport {
    replies.reverse();
    ScriptedTransport::responses(replies)
}

/// Zero-jitter test hooks, so waits are exact.
fn policy() -> (BusyPolicy, RecordingSleeper) {
    let clock = FakeClock::new();
    let sleeper = RecordingSleeper::new(clock.clone());
    (
        BusyPolicy::with_test_hooks(clock, sleeper.clone(), Duration::ZERO),
        sleeper,
    )
}

fn response(status: u16, headers: Vec<(&str, &str)>, body: Value) -> HttpResponse {
    text_response(
        status,
        headers,
        &serde_json::to_string(&body).expect("json"),
    )
}

fn text_response(status: u16, headers: Vec<(&str, &str)>, body: &str) -> HttpResponse {
    HttpResponse::new(
        status,
        headers
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect(),
        body.as_bytes().to_vec(),
    )
}

fn count_reply(tokens: i64) -> HttpResponse {
    response(200, Vec::new(), json!({ "input_tokens": tokens }))
}

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

fn haiku_message(text: &str, stop_reason: &str, input_tokens: i64) -> HttpResponse {
    response(
        200,
        Vec::new(),
        json!({
            "stop_reason": stop_reason,
            "content": [{ "type": "text", "text": text }],
            "usage": {
                "input_tokens": input_tokens,
                "cache_read_input_tokens": 2,
                "output_tokens": 9
            }
        }),
    )
}

fn luna_message(text: &str, status: &str, input_tokens: i64) -> HttpResponse {
    response(
        200,
        Vec::new(),
        json!({
            "status": status,
            "output": [
                { "type": "reasoning" },
                { "type": "message", "content": [{ "type": "output_text", "text": text }] }
            ],
            "usage": { "input_tokens": input_tokens }
        }),
    )
}

fn question(choices: &[&str]) -> Value {
    json!({
        "id": "q",
        "prompt": "judge this",
        "choices": choices,
    })
}

fn request_with(questions: Value, prompt: Option<&str>) -> JudgeRequest {
    let payload: CanonicalJudgePayload = serde_json::from_value(json!({
        "questions": questions,
        "state": [],
    }))
    .expect("payload");
    JudgeRequest::new(
        Arc::new(PayloadBundle::encode(&payload).expect("bundle")),
        prompt.map(LlmWireContext::new),
    )
}

fn standard_request(prompt: &str) -> JudgeRequest {
    request_with(json!([question(&["yes", "no"])]), Some(prompt))
}

fn credential() -> ResolvedApiKey {
    ResolvedApiKey::try_new(BackendKeyId::Anthropic, KEY.to_owned()).expect("key")
}

fn haiku(replies: Vec<HttpResponse>) -> (AnthropicHaikuBackend<ScriptedTransport>, Recorded) {
    let transport = script(replies);
    let (policy, sleeper) = policy();
    let backend =
        AnthropicHaikuBackend::new(credential(), transport.clone()).with_busy_policy(policy);
    (backend, Recorded { transport, sleeper })
}

fn luna(replies: Vec<HttpResponse>) -> (OpenAiLunaBackend<ScriptedTransport>, Recorded) {
    let transport = script(replies);
    let (policy, sleeper) = policy();
    let backend = OpenAiLunaBackend::new(credential(), transport.clone()).with_busy_policy(policy);
    (backend, Recorded { transport, sleeper })
}

/// The constructors are infallible (ADR-SAG-0020 r-1 amendment); the ids
/// come from the provider and the default transports exist.
#[test]
fn llm_backend_constructors_are_infallible() {
    assert_eq!(
        AnthropicHaikuBackend::new(credential(), script(Vec::new())).id(),
        BackendId::AnthropicHaiku
    );
    assert_eq!(
        OpenAiLunaBackend::new(credential(), script(Vec::new())).id(),
        BackendId::OpenAiLuna
    );
    assert_eq!(
        OpenAiLunaBackend::with_default_transport(credential()).id(),
        BackendId::OpenAiLuna
    );
    assert_eq!(
        AnthropicHaikuBackend::with_default_transport(credential()).id(),
        BackendId::AnthropicHaiku
    );
}

fn posts(recorded: &Recorded) -> Vec<HttpRequest> {
    recorded.transport.requests()
}

fn body_json(request: &HttpRequest) -> Value {
    serde_json::from_slice(request.body()).expect("request json")
}

fn url(request: &HttpRequest) -> &str {
    request.endpoint().as_str()
}

fn header<'a>(request: &'a HttpRequest, name: &str) -> &'a str {
    request
        .headers()
        .iter()
        .find(|(key, _)| key.as_str().eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
        .unwrap_or("")
}

/// The credential header: its name, its redacted form, and the key it holds.
fn assert_secret(request: &HttpRequest, name: &str, redacted: &str) {
    let secret = request.secret().expect("secret header");
    assert!(secret.name().as_str().eq_ignore_ascii_case(name));
    assert_eq!(secret.redacted(), redacted);
    assert!(secret.matches(KEY));
    assert_eq!(header(request, name), "", "the key is not a plain header");
}

fn assert_post(request: &HttpRequest) {
    assert_eq!(request.method().as_str(), "POST");
    assert!(request.endpoint().is_https());
}

fn assert_outcome_fields(outcome: &JudgeOutcome, backend: BackendId) {
    let value = serde_json::to_value(outcome).expect("outcome");
    let names: Vec<&str> = value
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(names, judge_outcome_field_names());
    assert_eq!(outcome.backend(), backend);
    assert!(outcome.provider_meta().is_some());
    assert!(
        outcome.input_sha256().is_none(),
        "backend-side hashing lands in r-2b"
    );
}

#[test]
fn test_haiku_request_embeds_canonical_payload() {
    let request = standard_request(PROMPT);
    let payload_text = std::str::from_utf8(request.payload().bytes()).expect("utf-8");
    let (backend, recorded) = haiku(vec![
        haiku_message(&answer_text(), "end_turn", 100),
        count_reply(100),
    ]);
    let outcome = backend.judge(&request).expect("judge");
    let seen = posts(&recorded);
    assert_eq!(seen.len(), 2);
    assert_post(&seen[0]);
    assert_post(&seen[1]);
    assert_eq!(
        url(&seen[0]),
        "https://api.anthropic.com/v1/messages/count_tokens"
    );
    assert_eq!(url(&seen[1]), "https://api.anthropic.com/v1/messages");
    assert_secret(&seen[1], "x-api-key", "x-api-key: [redacted]");
    assert_eq!(header(&seen[1], "anthropic-version"), "2026-10-21");
    assert_eq!(header(&seen[1], "content-type"), "application/json");

    let count = body_json(&seen[0]);
    let message = body_json(&seen[1]);
    assert_eq!(
        message["messages"],
        json!([{ "role": "user", "content": payload_text }])
    );
    assert_eq!(message["system"], PROMPT);
    assert_eq!(message["model"], "claude-haiku-5-5");
    assert_eq!(message["max_tokens"], 16_000);
    assert!(message.get("tools").is_none());
    assert!(message.get("output_config").is_none());
    let mut without_max = message.clone();
    without_max
        .as_object_mut()
        .expect("object")
        .remove("max_tokens");
    assert_eq!(count, without_max);
    assert_eq!(
        serde_json::to_value(&outcome.answers()["q"]).expect("answer"),
        json!({
            "choice": "no",
            "confidence": 0.7,
            "probabilities": { "no": 0.7, "yes": 0.3 }
        })
    );
    assert_eq!(outcome.provider_meta().expect("meta")["input_tokens"], 102);
    assert_eq!(outcome.provider_meta().expect("meta")["prompt_tokens"], 100);
    assert_eq!(outcome.provider_meta().expect("meta")["commands"], 0);
    assert_outcome_fields(&outcome, BackendId::AnthropicHaiku);
    let rendered = format!("{backend:?}");
    assert!(!rendered.contains(KEY));
}

#[test]
fn test_luna_request_embeds_canonical_payload() {
    let request = standard_request(PROMPT);
    let payload_text = std::str::from_utf8(request.payload().bytes()).expect("utf-8");
    let (backend, recorded) = luna(vec![
        luna_message(&answer_text(), "completed", 50),
        count_reply(50),
    ]);
    let outcome = backend.judge(&request).expect("judge");
    let seen = posts(&recorded);
    let message = body_json(&seen[1]);
    assert_eq!(
        message["input"],
        json!([{ "role": "user", "content": payload_text }])
    );
    assert_eq!(message["instructions"], PROMPT);
    assert_eq!(message["model"], "gpt-6-luna");
    assert!(message.get("tools").is_none());
    assert_eq!(body_json(&seen[0]), message);
    assert_outcome_fields(&outcome, BackendId::OpenAiLuna);
    assert!(!format!("{backend:?}").contains(KEY));
}

#[test]
fn test_luna_token_count_wire_before_judge() {
    let request = standard_request(PROMPT);
    let (backend, recorded) = luna(vec![
        luna_message(&answer_text(), "completed", 50),
        count_reply(50),
    ]);
    backend.judge(&request).expect("judge");
    let seen = posts(&recorded);
    assert_eq!(seen.len(), 2);
    assert_eq!(
        url(&seen[0]),
        "https://api.openai.com/v1/responses/input_tokens"
    );
    assert_eq!(url(&seen[1]), "https://api.openai.com/v1/responses");
    for request in &seen {
        assert_secret(request, "authorization", "Authorization: Bearer [redacted]");
    }
    assert_eq!(seen[0].body(), seen[1].body());
    assert_post(&seen[0]);
    assert_post(&seen[1]);
    let body = body_json(&seen[1]);
    assert_eq!(
        body.as_object().expect("object").keys().collect::<Vec<_>>(),
        ["input", "instructions", "model"]
    );
}

#[test]
fn test_llm_system_prompt_is_injected_not_hardcoded() {
    let request = standard_request(PROMPT);
    let (backend, recorded) = haiku(vec![
        haiku_message(&answer_text(), "end_turn", 10),
        count_reply(10),
    ]);
    backend.judge(&request).expect("judge");
    let message = body_json(&posts(&recorded)[1]);
    assert_eq!(message["system"], PROMPT);
    let source = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))
        .expect("src")
        .map(|entry| {
            let path = entry.expect("entry").path();
            std::fs::read_to_string(&path).unwrap_or_default()
        })
        .collect::<String>();
    assert!(!source.contains("strict data extraction utility"));
    assert!(
        !message["system"]
            .as_str()
            .expect("system")
            .contains("strict data extraction")
    );
}

#[test]
fn test_prompt_change_does_not_alter_prompt_sha256() {
    let first = standard_request("prompt-alpha");
    let second = standard_request("prompt-beta");
    assert_eq!(first.payload().sha256(), second.payload().sha256());
    assert_eq!(
        first.payload().sha256(),
        &Sha256Digest::of(first.payload().bytes())
    );
    let (backend, recorded) = haiku(vec![
        haiku_message(&answer_text(), "end_turn", 4),
        count_reply(4),
        haiku_message(&answer_text(), "end_turn", 4),
        count_reply(4),
    ]);
    backend.judge(&first).expect("first");
    backend.judge(&second).expect("second");
    assert_eq!(first.payload().sha256(), second.payload().sha256());
    let seen = posts(&recorded);
    let alpha = body_json(&seen[1]);
    let beta = body_json(&seen[3]);
    assert_eq!(alpha["messages"], beta["messages"]);
    assert_ne!(alpha["system"], beta["system"]);
    assert_eq!(alpha["system"], "prompt-alpha");
    assert_eq!(beta["system"], "prompt-beta");
    assert_ne!(first.payload().sha256(), &Sha256Digest::of(seen[1].body()));
}

#[test]
fn test_llm_input_is_refused_like_jev_and_nothing_is_sent() {
    let (backend, recorded) = haiku(vec![count_reply(1)]);
    let oversized = standard_request(&"x".repeat(100_000));
    let too_big = backend.judge(&oversized).expect_err("oversize");
    assert_eq!(too_big.code(), "INPUT_TOO_LARGE");
    assert!(posts(&recorded).is_empty());

    let short = request_with(json!([question(&["only"])]), Some(PROMPT));
    let invalid = backend.judge(&short).expect_err("one choice");
    assert_eq!(invalid.code(), "INPUT_INVALID");
    assert!(invalid.to_string().contains("two or more criteria"));
    assert!(posts(&recorded).is_empty());

    let missing_prompt = request_with(json!([question(&["yes", "no"])]), None);
    let missing = backend.judge(&missing_prompt).expect_err("prompt");
    assert_eq!(missing.code(), "INPUT_INVALID");
    assert!(posts(&recorded).is_empty());
}

#[test]
fn test_unknown_question_type_has_no_answer_schema() {
    let (backend, recorded) = haiku(vec![count_reply(1)]);
    // A question type outside the canonical payload is unrepresentable: it
    // never becomes a `PayloadBundle`, so no request reaches a backend.
    let mut ranked = question(&["a", "b"]);
    ranked["type"] = json!("rank");
    let parsed = serde_json::from_value::<CanonicalJudgePayload>(json!({
        "questions": [ranked],
        "state": [],
    }));
    assert!(parsed.is_err());
    let _ = backend.id();
    assert!(posts(&recorded).is_empty());
}

#[test]
fn test_llm_bad_reply_is_typed() {
    let replies = [
        "not json",
        r#"{"answers":[]}"#,
        r#"{"answers":{"q":{"choice":"maybe","confidence":0.7,"probabilities":{"yes":0.3,"no":0.7}}}}"#,
        r#"{"answers":{"q":{"choice":"no","confidence":0.7,"probabilities":{"yes":0.3,"no":0.7}},"x":{"choice":"no","confidence":0.7,"probabilities":{"yes":0.3,"no":0.7}}}}"#,
        r#"{"answers":{"q":{"choice":"no","confidence":1,"probabilities":{"no":1}}}}"#,
        r#"{"answers":{"q":{"choice":"no","confidence":true,"probabilities":{"yes":0.3,"no":0.7}}}}"#,
    ];
    for reply in replies {
        let (backend, recorded) = haiku(vec![haiku_message(reply, "end_turn", 1), count_reply(1)]);
        let error = backend.judge(&standard_request(PROMPT)).expect_err(reply);
        assert_eq!(error.code(), "RESPONSE_INVALID", "{reply} -> {error}");
        assert_eq!(posts(&recorded).len(), 2, "{reply}");
    }
}

#[test]
fn test_llm_stop_reason_truncation_is_response_invalid() {
    let (haiku_backend, haiku_recorded) = haiku(vec![
        haiku_message(&answer_text(), "max_tokens", 8),
        count_reply(8),
    ]);
    let haiku_error = haiku_backend
        .judge(&standard_request(PROMPT))
        .expect_err("truncated haiku");
    assert_eq!(haiku_error.code(), "RESPONSE_INVALID");
    assert!(haiku_error.to_string().contains("max_tokens"));
    assert!(haiku_error.to_string().contains("stop_reason"));
    assert_eq!(posts(&haiku_recorded).len(), 2);

    let (luna_backend, luna_recorded) = luna(vec![
        luna_message(&answer_text(), "incomplete", 8),
        count_reply(8),
    ]);
    let luna_error = luna_backend
        .judge(&standard_request(PROMPT))
        .expect_err("truncated luna");
    assert_eq!(luna_error.code(), "RESPONSE_INVALID");
    assert!(luna_error.to_string().contains("max_tokens"));
    assert!(luna_error.to_string().contains("incomplete"));
    assert_eq!(posts(&luna_recorded).len(), 2);
}

#[test]
fn test_llm_busy_retry_same_body() {
    let (backend, recorded) = haiku(vec![
        haiku_message(&answer_text(), "end_turn", 3),
        response(
            502,
            vec![("Retry-After", "soon")],
            json!({"error": "bad-gateway"}),
        ),
        response(529, Vec::new(), json!({"error": "overloaded"})),
        count_reply(3),
        response(
            429,
            vec![("Retry-After", "45")],
            json!({"error": "slow down"}),
        ),
    ]);
    backend.judge(&standard_request(PROMPT)).expect("retried");
    let seen = posts(&recorded);
    assert_eq!(seen.len(), 5);
    assert!(url(&seen[0]).ends_with("/count_tokens"));
    assert!(url(&seen[1]).ends_with("/count_tokens"));
    assert_eq!(seen[0].body(), seen[1].body());
    assert!(url(&seen[2]).ends_with("/v1/messages"));
    assert_eq!(seen[2].body(), seen[3].body());
    assert_eq!(seen[3].body(), seen[4].body());
    assert_ne!(seen[0].body(), seen[2].body());
    // Count call: Retry-After 45 clamps to 30 s. Judge call: 529 without
    // Retry-After is the first backoff (1 s); the malformed "soon" on the
    // 502 is treated as absent, so it is the second backoff (2 s).
    assert_eq!(
        recorded.delays(),
        [
            Duration::from_secs(30),
            Duration::from_secs(1),
            Duration::from_secs(2),
        ]
    );
}

#[test]
fn test_llm_busy_exhausted_wording_and_decimal_retry_after() {
    let busy = || {
        response(
            429,
            vec![("retry-after", "1.5")],
            json!({"error": "slow down"}),
        )
    };
    let (backend, recorded) = luna(vec![busy(), busy(), busy(), busy()]);
    let error = backend
        .judge(&standard_request(PROMPT))
        .expect_err("exhausted");
    assert_eq!(error.code(), "NETWORK_ERROR");
    assert_eq!(
        error.to_string(),
        "network error: luna HTTP 429 busy; 4 sends exhausted"
    );
    assert!(error.retryable());
    assert_eq!(posts(&recorded).len(), 4);
    assert_eq!(recorded.delays(), [Duration::from_millis(1_500); 3]);
}

/// Timeouts are transport construction state (ADR-SAG-0007): the default
/// transport uses 10 s / 120 s, and a caller passes its own `UreqTransport`.
#[test]
fn test_timeouts_are_transport_construction_state() {
    assert_eq!(Timeouts::default().connect(), Duration::from_secs(10));
    assert_eq!(Timeouts::default().request(), Duration::from_secs(120));
    let transport = UreqTransport::new(Timeouts::new(
        Duration::from_millis(250),
        Duration::from_secs(3),
    ));
    let backend = OpenAiLunaBackend::new(credential(), transport);
    assert_eq!(backend.id(), BackendId::OpenAiLuna);
    assert!(!format!("{backend:?}").contains(KEY));
}

#[test]
fn test_llm_non_2xx_detail_is_redacted() {
    let (backend, recorded) = haiku(vec![text_response(
        401,
        Vec::new(),
        &format!("invalid x-api-key {KEY}"),
    )]);
    let error = backend.judge(&standard_request(PROMPT)).expect_err("401");
    assert_eq!(error.code(), "NETWORK_ERROR");
    assert_eq!(
        error.to_string(),
        "network error: haiku HTTP 401: invalid x-api-key [redacted]"
    );
    assert_eq!(posts(&recorded).len(), 1);
}

#[test]
fn test_llm_non_busy_is_not_retried() {
    let (backend, recorded) = haiku(vec![response(
        400,
        Vec::new(),
        json!({"error": "bad request"}),
    )]);
    let error = backend.judge(&standard_request(PROMPT)).expect_err("400");
    assert_eq!(error.code(), "NETWORK_ERROR");
    assert_eq!(posts(&recorded).len(), 1);
    assert!(recorded.delays().is_empty());
}

#[test]
fn test_llm_bad_token_count_does_not_send_judge() {
    let (backend, recorded) = haiku(vec![response(200, Vec::new(), json!({}))]);
    let error = backend.judge(&standard_request(PROMPT)).expect_err("count");
    assert_eq!(error.code(), "RESPONSE_INVALID");
    let seen = posts(&recorded);
    assert_eq!(seen.len(), 1);
    assert!(url(&seen[0]).ends_with("/count_tokens"));
}

#[test]
fn test_llm_missing_key_does_not_build() {
    let error =
        ResolvedApiKey::try_new(BackendKeyId::Anthropic, String::new()).expect_err("missing");
    assert_eq!(error.code(), "KEY_MISSING");
    let control = ResolvedApiKey::try_new(BackendKeyId::OpenAi, format!("{KEY}\u{7}"))
        .expect_err("control character");
    assert_eq!(control.code(), "CONFIG_INVALID");
    assert!(!control.fix_hint().contains(KEY));
    let rendered = format!(
        "{:?}",
        OpenAiLunaBackend::new(credential(), script(Vec::new()))
    );
    assert!(!rendered.contains(KEY));
}

#[test]
fn backends_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<AnthropicHaikuBackend>();
    assert_send_sync::<OpenAiLunaBackend>();
    assert_send_sync::<OpenAiLunaBackend<ScriptedTransport>>();
}

#[test]
fn test_validate_for_send_refuses_oversize_before_http() {
    let (backend, recorded) = haiku(vec![count_reply(1)]);
    let oversized = standard_request(&"x".repeat(100_000));
    let error = backend.validate_for_send(&oversized).expect_err("validate");
    assert_eq!(error.code(), "INPUT_TOO_LARGE");
    let error = backend.judge(&oversized).expect_err("judge");
    assert_eq!(error.code(), "INPUT_TOO_LARGE");
    assert!(posts(&recorded).is_empty());
}

#[test]
fn test_provider_token_count_over_limit_does_not_send_judge() {
    let (backend, recorded) = haiku(vec![count_reply(32_001)]);
    backend
        .validate_for_send(&standard_request(PROMPT))
        .expect("local ceiling");
    let error = backend
        .judge(&standard_request(PROMPT))
        .expect_err("provider tokens");
    assert_eq!(error.code(), "INPUT_INVALID");
    assert!(error.to_string().contains("32001"));
    let seen = posts(&recorded);
    assert_eq!(seen.len(), 1);
    assert!(url(&seen[0]).ends_with("/count_tokens"));
}

#[test]
fn test_llm_text_reply_is_not_json_vendor_envelope() {
    let (backend, _) = haiku(vec![
        text_response(200, Vec::new(), "not-the-vendor-envelope"),
        count_reply(1),
    ]);
    let error = backend
        .judge(&standard_request(PROMPT))
        .expect_err("envelope");
    assert_eq!(error.code(), "RESPONSE_INVALID");
}
