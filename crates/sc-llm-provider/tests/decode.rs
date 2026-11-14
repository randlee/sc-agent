//! Decoding: both reply envelopes, JSON output, truncation, refusal, invalid bodies, non-2xx,
//! and one non-decision call sent through HttpClient over ScriptedTransport.
// Clippy allows unwrap, expect, panic and indexing only in test code (clippy.toml);
// the whole file, helpers included, is test code.
#![cfg(test)]

mod common;

use std::sync::Arc;

use common::{KEY, chat_message, haiku_message, key, response, text_response};
use sc_agent_http::testing::{ManualClock, ScriptedTransport};
use sc_agent_http::{Attempts, HttpClient, HttpResponse, LogContext, Next, ResendPolicy, Start};
use sc_agent_log::testing::MemoryLog;
use sc_agent_types::{Catalog, ErrorId, ModelChoice};
use sc_llm_provider::{
    LlmClient, LlmError, LlmProvider, LlmRequest, Message, Output, Part, Role, Usage,
};
use serde_json::json;

fn client(provider: LlmProvider) -> LlmClient {
    LlmClient::new(provider, key()).expect("client")
}

fn request(model: &str, output: Output) -> LlmRequest<'static> {
    LlmRequest {
        model: model.parse().expect("model"),
        system: None,
        messages: vec![Message {
            role: Role::User,
            content: vec![Part::Text("hello".to_owned())],
        }],
        max_tokens: 64,
        output,
    }
}

fn json_output() -> Output {
    Output::Json {
        name: "answer".to_owned(),
        schema: json!({ "type": "object" }),
    }
}

fn answer_text() -> String {
    json!({ "answers": { "q": { "noul": 0.123456789 } } }).to_string()
}

fn decode(
    provider: LlmProvider,
    output: Output,
    reply: HttpResponse,
) -> Result<sc_llm_provider::LlmResponse, LlmError> {
    client(provider).decode(
        &request("claude-haiku-5-5", output),
        reply,
        LogContext::NONE,
    )
}

/// AC 4: both wires, usage as reported, JSON output parsed into `json`.
#[test]
fn replies_decode_with_usage_and_json() {
    let anthropic = decode(
        LlmProvider::Anthropic,
        json_output(),
        haiku_message(&answer_text(), "end_turn", 8),
    )
    .expect("anthropic");
    assert_eq!(anthropic.model, "claude-haiku-5-5-20261007");
    assert_eq!(anthropic.text, answer_text());
    assert_eq!(anthropic.stop_reason.as_deref(), Some("end_turn"));
    // Provider's own counters; no cache sums.
    assert_eq!(
        anthropic.usage,
        Usage {
            input_tokens: 8,
            output_tokens: 9
        }
    );
    assert_eq!(
        anthropic.json,
        Some(json!({ "answers": { "q": { "noul": 0.123456789 } } }))
    );

    let chat = decode(
        LlmProvider::OpenAi,
        json_output(),
        chat_message(&answer_text(), "stop"),
    )
    .expect("chat");
    assert_eq!(chat.model, "gpt-6-luna-2026-09-22");
    assert_eq!(chat.stop_reason.as_deref(), Some("stop"));
    assert_eq!(
        chat.usage,
        Usage {
            input_tokens: 21,
            output_tokens: 5
        }
    );
    assert!(chat.json.is_some());

    let text = decode(
        LlmProvider::Anthropic,
        Output::Text,
        haiku_message("plain words", "end_turn", 1),
    )
    .expect("text");
    assert_eq!((text.text.as_str(), text.json), ("plain words", None));

    let blocks = response(
        200,
        Vec::new(),
        json!({
            "model": "m",
            "stop_reason": "end_turn",
            "content": [
                { "type": "text", "text": "one " },
                { "type": "tool_use", "id": "t" },
                { "type": "text", "text": "two" }
            ]
        }),
    );
    let joined = decode(LlmProvider::Anthropic, Output::Text, blocks).expect("blocks");
    assert_eq!(joined.text, "one two");
    assert_eq!(joined.usage, Usage::default());
}

/// AC 13: truncation is llm.truncated on both wires (ported from the old
/// `test_llm_stop_reason_truncation_is_response_invalid`).
#[test]
fn test_llm_stop_reason_truncation_is_truncated() {
    let haiku_error = decode(
        LlmProvider::Anthropic,
        json_output(),
        haiku_message(&answer_text(), "max_tokens", 8),
    )
    .expect_err("truncated haiku");
    assert_eq!(haiku_error.id(), "llm.truncated");
    assert!(matches!(
        &haiku_error,
        LlmError::Truncated { provider: LlmProvider::Anthropic, stop_reason } if stop_reason == "max_tokens"
    ));
    assert!(haiku_error.to_string().contains("max_tokens"));

    let luna_error = decode(
        LlmProvider::OpenAi,
        json_output(),
        chat_message(&answer_text(), "length"),
    )
    .expect_err("truncated luna");
    assert_eq!(luna_error.id(), "llm.truncated");
    assert!(luna_error.to_string().contains("length"));
}

/// AC 13: refusal is llm.refused on both wires.
#[test]
fn refusals_are_refused() {
    // Refusal text is provider-written: kept whole, with the key scrubbed (REQ-SAG-0002).
    let echoed = format!("I can't help with that. You sent {KEY}, which I will not use.");
    let anthropic = decode(
        LlmProvider::Anthropic,
        json_output(),
        haiku_message(&echoed, "refusal", 3),
    )
    .expect_err("refusal");
    assert_eq!(anthropic.id(), "llm.refused");
    assert!(matches!(
        &anthropic,
        LlmError::Refused { message, .. }
            if message == "I can't help with that. You sent [redacted], which I will not use."
    ));
    let rendered = format!("{anthropic} {anthropic:?} {:?}", anthropic.detail());
    assert!(!rendered.contains(KEY), "{rendered}");

    let chat = response(
        200,
        Vec::new(),
        json!({
            "model": "gpt-6-luna",
            "choices": [{
                "message": { "role": "assistant", "content": null, "refusal": "No." },
                "finish_reason": "stop"
            }]
        }),
    );
    let error = decode(LlmProvider::OpenAi, json_output(), chat).expect_err("refusal");
    assert!(matches!(&error, LlmError::Refused { message, .. } if message == "No."));
}

/// AC 13: bodies that are not the envelope, no text, text that is not JSON.
#[test]
fn invalid_bodies() {
    let not_json = text_response(200, Vec::new(), "<html>gateway</html>");
    let error = decode(LlmProvider::Anthropic, json_output(), not_json).expect_err("html");
    assert_eq!(error.id(), "llm.reply_invalid");
    assert!(!error.to_string().contains("gateway"), "{error}");

    for body in [
        json!({ "content": [] }),
        json!({ "model": "m" }),
        json!({ "model": "m", "content": [], "usage": { "input_tokens": -1 } }),
    ] {
        let error = decode(
            LlmProvider::Anthropic,
            json_output(),
            response(200, Vec::new(), body.clone()),
        )
        .expect_err("envelope");
        assert_eq!(error.id(), "llm.reply_invalid", "{body}");
    }
    let no_choices = response(200, Vec::new(), json!({ "model": "m", "choices": [] }));
    assert_eq!(
        decode(LlmProvider::OpenRouter, json_output(), no_choices)
            .expect_err("choices")
            .id(),
        "llm.reply_invalid"
    );

    let no_text = response(
        200,
        Vec::new(),
        json!({ "model": "m", "stop_reason": "end_turn", "content": [] }),
    );
    assert_eq!(
        decode(LlmProvider::Anthropic, json_output(), no_text)
            .expect_err("no text")
            .id(),
        "llm.no_text"
    );

    let prose = decode(
        LlmProvider::Anthropic,
        json_output(),
        haiku_message("Sure! Here is the JSON", "end_turn", 1),
    )
    .expect_err("prose");
    assert_eq!(prose.id(), "llm.output_not_json");
}

/// AC 13, AC 15: a 401 is llm.api_status with the provider's message and no key.
#[test]
fn non_2xx_is_api_status_without_the_key() {
    let reply = response(
        401,
        vec![("request-id", "req_123")],
        json!({
            "type": "error",
            "error": {
                "type": "authentication_error",
                "message": format!("invalid x-api-key {KEY}")
            }
        }),
    );
    let error = decode(LlmProvider::Anthropic, json_output(), reply).expect_err("401");
    assert_eq!(error.id(), "llm.api_status");
    let LlmError::ApiStatus { provider, reply } = &error else {
        panic!("not ApiStatus: {error:?}");
    };
    assert_eq!(*provider, LlmProvider::Anthropic);
    assert_eq!(reply.status, 401);
    assert_eq!(reply.message, "invalid x-api-key [redacted]");
    assert_eq!(reply.request_id.as_deref(), Some("req_123"));
    let rendered = format!("{error} {error:?} {:?}", error.detail());
    assert!(rendered.contains("invalid x-api-key"));
    assert!(!rendered.contains(KEY), "{rendered}");
}

/// AC 3 (REQ-SAG-0502): a non-decision request for a model chosen by short name, sent
/// through HttpClient::may_start and send_once over ScriptedTransport.
#[test]
fn non_decision_request_through_http_client() {
    let model = Catalog::builtin().resolve(&"haiku".parse::<ModelChoice>().expect("short"));
    let llm = client(LlmProvider::Anthropic);
    let call = LlmRequest {
        model,
        system: Some("Summarize in five words.".to_owned()),
        messages: vec![Message {
            role: Role::User,
            content: vec![Part::Text("A long release note.".to_owned())],
        }],
        max_tokens: 32,
        output: Output::Text,
    };
    let http = llm.encode(&call).expect("encode");

    let transport = ScriptedTransport::responses(vec![haiku_message(
        "Release adds five new things.",
        "end_turn",
        14,
    )]);
    let memory = MemoryLog::new().expect("memory log");
    let client = HttpClient::new(
        Arc::new(transport.clone()),
        ResendPolicy::default(),
        memory.general(),
    );
    let clock = ManualClock::new();
    let now = clock.clock();
    let Start::Go(permit) = client
        .may_start(&http, Attempts::new(), now.now())
        .expect("admitted")
    else {
        panic!("a fresh request to an unpaused host may start");
    };
    let Next::Done { response, .. } = client
        .send_once(&http, permit, LogContext::NONE, &now)
        .expect("sent")
    else {
        panic!("200 is a final reply");
    };
    let reply = llm
        .decode(&call, response, LogContext::NONE)
        .expect("decode");
    assert_eq!(reply.text, "Release adds five new things.");

    let sent = transport.requests();
    assert_eq!(sent.len(), 1);
    let body: serde_json::Value = serde_json::from_slice(sent[0].body()).expect("body");
    assert_eq!(body["model"], "claude-haiku-5-5");
    assert_eq!(body["system"], "Summarize in five words.");
}

/// A send failure wrapped by http_error keeps the HttpError whole.
#[test]
fn http_error_wraps_the_send_failure() {
    let transport = ScriptedTransport::new(Vec::new());
    let llm = client(LlmProvider::OpenAi);
    let http = llm
        .encode(&request("gpt-6-luna", Output::Text))
        .expect("encode");
    let send = sc_agent_http::Transport::send(&transport, &http).expect_err("exhausted");
    let error = llm.http_error(send.clone());
    assert_eq!(error.id(), "llm.http");
    let cause = error.detail().cause.expect("cause");
    assert_eq!(cause.id, "http.network");
    assert!(matches!(&error, LlmError::Http { source, .. } if *source == send));
}

/// A malformed envelope names the field; it never copies the untrusted value.
#[test]
fn invalid_envelope_names_the_field_without_echoing_the_credential() {
    let client = client(LlmProvider::Anthropic);
    let reply = response(200, Vec::new(), json!({"model":"haiku", "content": KEY}));
    let error = client
        .decode(&request("haiku", json_output()), reply, LogContext::NONE)
        .expect_err("content must be an array");
    assert_eq!(error.id(), "llm.reply_invalid");
    assert!(
        matches!(&error, LlmError::ReplyInvalid { message, .. } if message == "the reply has no `content` array")
    );
    assert!(
        !serde_json::to_string(&error.detail())
            .expect("detail")
            .contains(KEY)
    );
}
