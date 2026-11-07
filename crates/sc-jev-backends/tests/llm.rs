//! Fake-transport coverage for Haiku and Luna (sprint a-5).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use sc_agent_types::test_util::judge_outcome_field_names;
use sc_agent_types::{
    BackendId, BackendKeyId, JevError, JudgeBackend, JudgeOutcome, JudgeRequest, LlmWireContext,
    PayloadBundle, ResolvedApiKey, Sha256Digest,
};
use sc_jev_backends::{
    AnthropicHaikuBackend, BackendTransport, DEFAULT_CONNECT_TIMEOUT, DEFAULT_REQUEST_TIMEOUT,
    HttpRequest, HttpResponse, OpenAiLunaBackend, RetrySleeper,
};
use serde_json::{Value, json};

const PROMPT: &str = "INJECTED-EVALUATOR-PROMPT";
const KEY: &str = "test-api-key";

struct Recorded {
    posts: Mutex<Vec<HttpRequest>>,
    replies: Mutex<Vec<HttpResponse>>,
    delays: Mutex<Vec<Duration>>,
}

struct ScriptedTransport {
    recorded: Arc<Recorded>,
}

struct ScriptedSleeper {
    recorded: Arc<Recorded>,
}

impl ScriptedTransport {
    fn new(replies: Vec<HttpResponse>) -> (Self, Arc<Recorded>) {
        let recorded = Arc::new(Recorded {
            posts: Mutex::new(Vec::new()),
            replies: Mutex::new(replies),
            delays: Mutex::new(Vec::new()),
        });
        (
            Self {
                recorded: Arc::clone(&recorded),
            },
            recorded,
        )
    }
}

impl BackendTransport for ScriptedTransport {
    fn send(&self, mut request: HttpRequest) -> Result<HttpResponse, JevError> {
        // The sending transport is the only reader of the sealed key.
        request.install_auth();
        // The last queued reply is the next response.
        self.recorded.posts.lock().expect("posts").push(request);
        self.recorded
            .replies
            .lock()
            .expect("replies")
            .pop()
            .ok_or_else(|| JevError::NetworkError {
                detail: "script exhausted".into(),
            })
    }
}

impl RetrySleeper for ScriptedSleeper {
    fn wait(&self, delay: Duration) {
        self.recorded.delays.lock().expect("delays").push(delay);
    }
}

fn response(status: u16, headers: Vec<(&str, &str)>, body: Value) -> HttpResponse {
    HttpResponse {
        status,
        headers: headers
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect(),
        body: serde_json::to_vec(&body).expect("json"),
    }
}

fn text_response(status: u16, headers: Vec<(&str, &str)>, body: &str) -> HttpResponse {
    HttpResponse {
        status,
        headers: headers
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect(),
        body: body.as_bytes().to_vec(),
    }
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
    let bytes = sc_agent_types::canonical_json_bytes(&json!({
        "questions": questions,
        "state": [],
    }))
    .expect("canonical");
    JudgeRequest {
        payload: PayloadBundle::from_canonical_bytes(bytes),
        llm: prompt.map(|system_prompt| LlmWireContext {
            system_prompt: system_prompt.to_owned(),
        }),
    }
}

fn standard_request(prompt: &str) -> JudgeRequest {
    request_with(json!([question(&["yes", "no"])]), Some(prompt))
}

fn credential() -> ResolvedApiKey {
    ResolvedApiKey::try_new(BackendKeyId::Anthropic, KEY.to_owned()).expect("key")
}

/// Largest jitter the busy-retry policy adds to a wait.
const JITTER_MAX: Duration = Duration::from_millis(250);

/// Asserts each recorded wait is `expected` plus at most the policy jitter.
fn assert_waits(actual: &[Duration], expected: &[Duration]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?} vs {expected:?}");
    for (got, want) in actual.iter().zip(expected) {
        assert!(
            *got >= *want && *got <= *want + JITTER_MAX,
            "{actual:?} vs {expected:?}"
        );
    }
}

fn haiku(replies: Vec<HttpResponse>) -> (AnthropicHaikuBackend, Arc<Recorded>) {
    let (transport, recorded) = ScriptedTransport::new(replies);
    let sleeper = ScriptedSleeper {
        recorded: Arc::clone(&recorded),
    };
    let backend = AnthropicHaikuBackend::new(credential(), transport)
        .expect("key")
        .with_retry_sleeper(sleeper);
    (backend, recorded)
}

fn luna(replies: Vec<HttpResponse>) -> (OpenAiLunaBackend, Arc<Recorded>) {
    let (transport, recorded) = ScriptedTransport::new(replies);
    let sleeper = ScriptedSleeper {
        recorded: Arc::clone(&recorded),
    };
    let backend = OpenAiLunaBackend::new(credential(), transport)
        .expect("key")
        .with_retry_sleeper(sleeper);
    (backend, recorded)
}

fn posts(recorded: &Recorded) -> Vec<HttpRequest> {
    recorded.posts.lock().expect("posts").clone()
}

fn body_json(request: &HttpRequest) -> Value {
    serde_json::from_slice(&request.body).expect("request json")
}

fn header<'a>(request: &'a HttpRequest, name: &str) -> &'a str {
    request
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
        .unwrap_or("")
}

fn assert_default_timeouts(request: &HttpRequest) {
    assert_eq!(request.method, "POST");
    assert_eq!(request.connect_timeout, DEFAULT_CONNECT_TIMEOUT);
    assert_eq!(request.request_timeout, DEFAULT_REQUEST_TIMEOUT);
    assert_eq!(DEFAULT_CONNECT_TIMEOUT, Duration::from_secs(10));
    assert_eq!(DEFAULT_REQUEST_TIMEOUT, Duration::from_secs(120));
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
    assert_eq!(outcome.backend, backend);
    assert!(outcome.probabilities.is_none());
    assert!(outcome.provider_meta.is_some());
}

#[test]
fn test_haiku_request_embeds_canonical_payload() {
    let request = standard_request(PROMPT);
    let payload_text = std::str::from_utf8(request.payload.bytes()).expect("utf-8");
    let (backend, recorded) = haiku(vec![
        haiku_message(&answer_text(), "end_turn", 100),
        count_reply(100),
    ]);
    let outcome = backend.judge(&request).expect("judge");
    let seen = posts(&recorded);
    assert_eq!(seen.len(), 2);
    assert_default_timeouts(&seen[0]);
    assert_default_timeouts(&seen[1]);
    assert_eq!(
        seen[0].url,
        "https://api.anthropic.com/v1/messages/count_tokens"
    );
    assert_eq!(seen[1].url, "https://api.anthropic.com/v1/messages");
    assert_eq!(header(&seen[1], "x-api-key"), KEY);
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
        outcome.answers["q"],
        json!({
            "choice": "no",
            "confidence": 0.7,
            "probabilities": { "no": 0.7, "yes": 0.3 }
        })
    );
    assert_eq!(
        outcome.provider_meta.as_ref().expect("meta")["input_tokens"],
        102
    );
    assert_eq!(
        outcome.provider_meta.as_ref().expect("meta")["prompt_tokens"],
        100
    );
    assert_eq!(outcome.provider_meta.as_ref().expect("meta")["commands"], 0);
    assert_outcome_fields(&outcome, BackendId::AnthropicHaiku);
    let rendered = format!("{backend:?}");
    assert!(!rendered.contains(KEY));
}

#[test]
fn test_luna_request_embeds_canonical_payload() {
    let request = standard_request(PROMPT);
    let payload_text = std::str::from_utf8(request.payload.bytes()).expect("utf-8");
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
        seen[0].url,
        "https://api.openai.com/v1/responses/input_tokens"
    );
    assert_eq!(seen[1].url, "https://api.openai.com/v1/responses");
    assert_eq!(header(&seen[0], "authorization"), format!("Bearer {KEY}"));
    assert_eq!(header(&seen[1], "authorization"), format!("Bearer {KEY}"));
    assert_eq!(seen[0].body, seen[1].body);
    assert_default_timeouts(&seen[0]);
    assert_default_timeouts(&seen[1]);
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
    assert_eq!(first.payload.sha256(), second.payload.sha256());
    assert_eq!(
        first.payload.sha256(),
        &Sha256Digest::of(first.payload.bytes())
    );
    let (backend, recorded) = haiku(vec![
        haiku_message(&answer_text(), "end_turn", 4),
        count_reply(4),
        haiku_message(&answer_text(), "end_turn", 4),
        count_reply(4),
    ]);
    backend.judge(&first).expect("first");
    backend.judge(&second).expect("second");
    assert_eq!(first.payload.sha256(), second.payload.sha256());
    let seen = posts(&recorded);
    let alpha = body_json(&seen[1]);
    let beta = body_json(&seen[3]);
    assert_eq!(alpha["messages"], beta["messages"]);
    assert_ne!(alpha["system"], beta["system"]);
    assert_eq!(alpha["system"], "prompt-alpha");
    assert_eq!(beta["system"], "prompt-beta");
    assert_ne!(first.payload.sha256(), &Sha256Digest::of(&seen[1].body));
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
    let mut ranked = question(&["a", "b"]);
    ranked["type"] = json!("rank");
    let request = request_with(json!([ranked]), Some(PROMPT));
    let error = backend.judge(&request).expect_err("rank");
    assert_eq!(error.code(), "INPUT_INVALID");
    assert!(error.to_string().contains("no answer schema"));
    assert!(error.to_string().contains("rank"));
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
    assert!(seen[0].url.ends_with("/count_tokens"));
    assert!(seen[1].url.ends_with("/count_tokens"));
    assert_eq!(seen[0].body, seen[1].body);
    assert!(seen[2].url.ends_with("/v1/messages"));
    assert_eq!(seen[2].body, seen[3].body);
    assert_eq!(seen[3].body, seen[4].body);
    assert_ne!(seen[0].body, seen[2].body);
    let delays = recorded.delays.lock().expect("delays").clone();
    assert_waits(
        &delays,
        &[
            Duration::from_secs(30),
            Duration::from_secs(1),
            Duration::from_secs(30),
        ],
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
        "network error: luna busy: HTTP 429 after 4 sends; retry attempts exhausted"
    );
    assert_eq!(posts(&recorded).len(), 4);
    let delays = recorded.delays.lock().expect("delays").clone();
    assert_waits(&delays, &[Duration::from_millis(1_500); 3]);
}

#[test]
fn test_with_timeouts_reaches_every_request() {
    let (transport, recorded) = ScriptedTransport::new(vec![
        luna_message(&answer_text(), "completed", 5),
        count_reply(5),
    ]);
    let backend = OpenAiLunaBackend::new(credential(), transport)
        .expect("key")
        .with_timeouts(Duration::from_millis(250), Duration::from_secs(3));
    backend.judge(&standard_request(PROMPT)).expect("judge");
    let seen = posts(&recorded);
    assert_eq!(seen.len(), 2);
    for request in &seen {
        assert_eq!(request.connect_timeout, Duration::from_millis(250));
        assert_eq!(request.request_timeout, Duration::from_secs(3));
    }
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
    assert!(recorded.delays.lock().expect("delays").is_empty());
}

#[test]
fn test_llm_bad_token_count_does_not_send_judge() {
    let (backend, recorded) = haiku(vec![response(200, Vec::new(), json!({}))]);
    let error = backend.judge(&standard_request(PROMPT)).expect_err("count");
    assert_eq!(error.code(), "RESPONSE_INVALID");
    let seen = posts(&recorded);
    assert_eq!(seen.len(), 1);
    assert!(seen[0].url.ends_with("/count_tokens"));
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
        OpenAiLunaBackend::new(credential(), {
            let (transport, _) = ScriptedTransport::new(Vec::new());
            transport
        })
        .expect("luna")
    );
    assert!(!rendered.contains(KEY));
}

#[test]
fn backends_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<AnthropicHaikuBackend>();
    assert_send_sync::<OpenAiLunaBackend>();
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
    assert!(seen[0].url.ends_with("/count_tokens"));
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
