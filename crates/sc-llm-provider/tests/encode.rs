//! Encoding: provider model ids, bodies, headers, JSON-schema output, images, model lists.
// Clippy allows unwrap, expect, panic and indexing only in test code (clippy.toml);
// the whole file, helpers included, is test code.
#![cfg(test)]

mod common;

use common::{KEY, key, response};
use sc_agent_http::{Endpoint, HttpRequest, Method};
use sc_agent_types::{ErrorId, MODELS, ModelId, Provider};
use sc_llm_provider::{
    LlmClient, LlmError, LlmProvider, LlmRequest, Message, Output, Part, Role, Wire,
    catalog_model_id, provider_model_id,
};
use serde_json::{Value, json};

fn client(provider: LlmProvider) -> LlmClient {
    LlmClient::new(provider, key()).expect("client")
}

fn request(model: &str, output: Output) -> LlmRequest {
    LlmRequest {
        model: model.parse().expect("model"),
        system: Some("be brief".to_owned()),
        messages: vec![Message {
            role: Role::User,
            content: vec![Part::Text("hello".to_owned())],
        }],
        max_tokens: 321,
        output,
    }
}

fn body(request: &HttpRequest) -> Value {
    serde_json::from_slice(request.body()).expect("json body")
}

fn header<'a>(request: &'a HttpRequest, name: &str) -> Option<&'a str> {
    request
        .headers()
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn assert_no_key(request: &HttpRequest) {
    let rendered = format!(
        "{request:?} {} {:?}",
        String::from_utf8_lossy(request.body()),
        request.headers()
    );
    assert!(!rendered.contains(KEY), "{rendered}");
}

fn schema_output() -> Output {
    Output::Json {
        name: "answer".to_owned(),
        schema: json!({
            "type": "object",
            "required": ["ok"],
            "additionalProperties": false,
            "properties": { "ok": { "type": "boolean" } }
        }),
    }
}

/// AC 2 (REQ-SAG-0103): one catalog id through two providers, each sending its own id.
#[test]
fn one_catalog_id_selects_the_model_through_two_providers() {
    let anthropic = client(LlmProvider::Anthropic)
        .encode(&request("claude-haiku-5-5", Output::Text))
        .expect("encode");
    let openrouter = client(LlmProvider::OpenRouter)
        .encode(&request("claude-haiku-5-5", Output::Text))
        .expect("encode");
    assert_eq!(body(&anthropic)["model"], "claude-haiku-5-5");
    assert_eq!(body(&openrouter)["model"], "anthropic/claude-haiku-5.5");
    assert_ne!(anthropic.endpoint(), openrouter.endpoint());
    assert_eq!(
        openrouter.endpoint().as_str(),
        "https://openrouter.ai/api/v1/chat/completions"
    );
    assert_eq!(LlmProvider::OpenAi.as_str(), "openai");
    assert_eq!(Provider::from(LlmProvider::OpenAi).to_string(), "openai");
}

/// AC 2: catalog_model_id inverts provider_model_id for every MODELS row and provider.
#[test]
fn catalog_model_id_inverts_provider_model_id() {
    for row in MODELS {
        for provider in LlmProvider::ALL.iter().copied() {
            let sent = provider_model_id(provider, row.model());
            assert_eq!(
                catalog_model_id(provider, &sent),
                row.model().as_str(),
                "{provider} {sent}"
            );
        }
    }
    let luna: ModelId = "gpt-6-luna".parse().expect("model");
    assert_eq!(
        provider_model_id(LlmProvider::OpenRouter, &luna),
        "openai/gpt-6-luna"
    );
    assert_eq!(
        provider_model_id(
            LlmProvider::Together,
            &"claude-haiku-5-5".parse().expect("m")
        ),
        "anthropic/claude-haiku-5-5"
    );
    assert_eq!(provider_model_id(LlmProvider::Nous, &luna), "gpt-6-luna");
}

/// AC 3: Anthropic Messages body and headers; the key only in the SecretHeader.
#[test]
fn anthropic_body_and_headers() {
    let http = client(LlmProvider::Anthropic)
        .encode(&request("claude-haiku-5-5", Output::Text))
        .expect("encode");
    assert_eq!(http.method(), Method::Post);
    assert_eq!(
        http.endpoint().as_str(),
        "https://api.anthropic.com/v1/messages"
    );
    assert_eq!(header(&http, "anthropic-version"), Some("2026-10-21"));
    assert_eq!(header(&http, "content-type"), Some("application/json"));
    let secret = http.secret().expect("secret");
    assert_eq!(secret.name(), "x-api-key");
    assert_eq!(secret.redacted(), "x-api-key: [redacted]");
    assert_eq!(
        body(&http),
        json!({
            "model": "claude-haiku-5-5",
            "max_tokens": 321,
            "system": "be brief",
            "messages": [{ "role": "user", "content": [{ "type": "text", "text": "hello" }] }]
        })
    );
    assert_no_key(&http);
}

/// AC 3: Chat Completions body and Bearer; openai `max_completion_tokens`, others `max_tokens`;
/// Output::Text has no format field.
#[test]
fn chat_body_bearer_and_token_field() {
    let openai = client(LlmProvider::OpenAi)
        .encode(&request("gpt-6-luna", Output::Text))
        .expect("encode");
    assert_eq!(
        openai.endpoint().as_str(),
        "https://api.openai.com/v1/chat/completions"
    );
    assert_eq!(
        openai.secret().map(|secret| secret.redacted()),
        Some("Authorization: Bearer [redacted]".to_owned())
    );
    assert_eq!(header(&openai, "content-type"), Some("application/json"));
    assert_eq!(header(&openai, "anthropic-version"), None);
    assert_eq!(
        body(&openai),
        json!({
            "model": "gpt-6-luna",
            "max_completion_tokens": 321,
            "messages": [
                { "role": "system", "content": "be brief" },
                { "role": "user", "content": [{ "type": "text", "text": "hello" }] }
            ]
        })
    );
    assert_no_key(&openai);
    for provider in [
        LlmProvider::OpenRouter,
        LlmProvider::Together,
        LlmProvider::Nous,
    ] {
        let http = client(provider)
            .encode(&request("gpt-6-luna", Output::Text))
            .expect("encode");
        let body = body(&http);
        assert_eq!(body["max_tokens"], 321, "{provider}");
        assert!(body.get("max_completion_tokens").is_none(), "{provider}");
        assert!(body.get("response_format").is_none(), "{provider}");
        assert_no_key(&http);
    }
    let nous = client(LlmProvider::Nous)
        .encode(&request("hermes-5", Output::Text))
        .expect("encode");
    assert_eq!(
        nous.endpoint().as_str(),
        "https://inference-api.nousresearch.com/v1/chat/completions"
    );
    let together = client(LlmProvider::Together)
        .encode(&request("gpt-6-luna", Output::Text))
        .expect("encode");
    assert_eq!(
        together.endpoint().as_str(),
        "https://api.together.ai/v1/chat/completions"
    );
}

/// AC 3: from_provider is None only for TypeSafe; a client builds for every LlmProvider.
#[test]
fn every_llm_provider_has_a_client() {
    for provider in Provider::ALL.iter().copied() {
        let llm = LlmProvider::from_provider(provider);
        assert_eq!(llm.is_none(), provider == Provider::TypeSafe, "{provider}");
        if let Some(llm) = llm {
            assert_eq!(Provider::from(llm), provider);
            let client = client(llm);
            assert_eq!(client.provider(), llm);
            assert_eq!(client.wire(), Wire::of(llm));
            let rendered = format!("{client:?}");
            assert!(rendered.contains("<redacted>") && rendered.contains("<unset>"));
            assert!(!rendered.contains(KEY), "{rendered}");
        }
    }
    assert_eq!(Wire::of(LlmProvider::Anthropic), Wire::AnthropicMessages);
}

/// AC 4: Output::Json as output_config.format json_schema and response_format strict.
#[test]
fn json_output_uses_structured_output() {
    let anthropic = body(
        &client(LlmProvider::Anthropic)
            .encode(&request("claude-haiku-5-5", schema_output()))
            .expect("encode"),
    );
    assert_eq!(anthropic["output_config"]["format"]["type"], "json_schema");
    assert_eq!(
        anthropic["output_config"]["format"]["schema"]["required"],
        json!(["ok"])
    );
    for provider in [LlmProvider::OpenAi, LlmProvider::OpenRouter] {
        let chat = body(
            &client(provider)
                .encode(&request("gpt-6-luna", schema_output()))
                .expect("encode"),
        );
        let format = &chat["response_format"];
        assert_eq!(format["type"], "json_schema");
        assert_eq!(format["json_schema"]["strict"], true);
        assert_eq!(format["json_schema"]["name"], "answer");
        assert_eq!(format["json_schema"]["schema"]["required"], json!(["ok"]));
    }
}

/// AC 8: Part::Image as an Anthropic image block and a Chat image_url data URL.
#[test]
fn images_on_both_wires() {
    let mut with_image = request("claude-haiku-5-5", Output::Text);
    with_image.messages = vec![Message {
        role: Role::User,
        content: vec![
            Part::Text("look".to_owned()),
            Part::Image {
                media_type: "image/png",
                data_base64: "iVBORw0KGgo=".to_owned(),
            },
        ],
    }];
    let anthropic = body(
        &client(LlmProvider::Anthropic)
            .encode(&with_image)
            .expect("encode"),
    );
    assert_eq!(
        anthropic["messages"][0]["content"][1],
        json!({
            "type": "image",
            "source": { "type": "base64", "media_type": "image/png", "data": "iVBORw0KGgo=" }
        })
    );
    let chat = body(
        &client(LlmProvider::OpenAi)
            .encode(&with_image)
            .expect("encode"),
    );
    assert_eq!(
        chat["messages"][1]["content"][1],
        json!({
            "type": "image_url",
            "image_url": { "url": "data:image/png;base64,iVBORw0KGgo=" }
        })
    );
}

/// The key may not go to a plain-http host that is not loopback (a with_endpoint override).
#[test]
fn plain_http_override_is_refused() {
    let endpoint = Endpoint::parse_local("http://gateway.example.test/v1/messages").expect("url");
    let error = client(LlmProvider::Anthropic)
        .with_endpoint(endpoint)
        .encode(&request("claude-haiku-5-5", Output::Text))
        .expect_err("plain http");
    assert_eq!(error.id(), "llm.http");
    assert!(
        matches!(&error, LlmError::Http { source, .. } if source.id() == "http.key_on_plain_http")
    );
    assert!(!format!("{error} {error:?}").contains(KEY));
}

/// AC 10: models_request is GET {base}/models; decode_models reads both list shapes.
#[test]
fn models_request_and_both_list_shapes() {
    let anthropic = client(LlmProvider::Anthropic);
    let get = anthropic.models_request().expect("models");
    assert_eq!(get.method(), Method::Get);
    assert_eq!(
        get.endpoint().as_str(),
        "https://api.anthropic.com/v1/models"
    );
    assert_eq!(get.secret().map(|s| s.name()), Some("x-api-key"));
    assert_eq!(header(&get, "anthropic-version"), Some("2026-10-21"));
    assert!(get.body().is_empty());
    assert_no_key(&get);
    let openrouter = client(LlmProvider::OpenRouter);
    let get = openrouter.models_request().expect("models");
    assert_eq!(
        get.endpoint().as_str(),
        "https://openrouter.ai/api/v1/models"
    );
    assert_eq!(get.secret().map(|s| s.name()), Some("Authorization"));
    for (provider, url) in [
        (LlmProvider::OpenAi, "https://api.openai.com/v1/models"),
        (LlmProvider::Together, "https://api.together.ai/v1/models"),
        (
            LlmProvider::Nous,
            "https://inference-api.nousresearch.com/v1/models",
        ),
    ] {
        let get = client(provider).models_request().expect("models");
        assert_eq!(get.endpoint().as_str(), url);
    }

    let data = response(
        200,
        Vec::new(),
        json!({ "data": [{ "id": "claude-haiku-5-5", "type": "model" }, { "id": "claude-opus-5-5" }] }),
    );
    assert_eq!(
        anthropic.decode_models(data).expect("data"),
        ["claude-haiku-5-5", "claude-opus-5-5"]
    );
    let array = response(
        200,
        Vec::new(),
        json!([{ "id": "openai/gpt-6-luna" }, { "id": "anthropic/claude-haiku-5.5" }]),
    );
    assert_eq!(
        client(LlmProvider::Together)
            .decode_models(array)
            .expect("array"),
        ["openai/gpt-6-luna", "anthropic/claude-haiku-5.5"]
    );
    let refused = response(
        401,
        Vec::new(),
        json!({ "error": { "type": "authentication_error", "message": format!("bad key {KEY}") } }),
    );
    let error = anthropic.decode_models(refused).expect_err("401");
    assert_eq!(error.id(), "llm.api_status");
    assert!(!format!("{error} {error:?}").contains(KEY));
    let shapeless = response(200, Vec::new(), json!({ "models": [] }));
    assert_eq!(
        anthropic.decode_models(shapeless).expect_err("shape").id(),
        "llm.reply_invalid"
    );
}
