//! Request bodies of the two wires.

use serde_json::{Map, Value, json};

use crate::{LlmProvider, LlmRequest, Message, Output, Part, Wire, provider_model_id};

/// The JSON body of `request` for `provider`.
pub(crate) fn body(provider: LlmProvider, request: &LlmRequest) -> Value {
    match Wire::of(provider) {
        Wire::AnthropicMessages => anthropic(provider, request),
        Wire::ChatCompletions => chat(provider, request),
    }
}

/// `{"model","max_tokens","system"?,"messages","output_config"?}`.
fn anthropic(provider: LlmProvider, request: &LlmRequest) -> Value {
    let mut body = Map::new();
    body.insert(
        "model".to_owned(),
        provider_model_id(provider, &request.model).into(),
    );
    body.insert("max_tokens".to_owned(), request.max_tokens.into());
    if let Some(system) = &request.system {
        body.insert("system".to_owned(), system.clone().into());
    }
    body.insert(
        "messages".to_owned(),
        request
            .messages
            .iter()
            .map(|message| message_value(message, anthropic_part))
            .collect(),
    );
    if let Output::Json { schema, .. } = &request.output {
        body.insert(
            "output_config".to_owned(),
            json!({ "format": { "type": "json_schema", "schema": schema } }),
        );
    }
    Value::Object(body)
}

/// `{"model","max_completion_tokens"|"max_tokens","messages","response_format"?}`; the system
/// prompt is the first message.
fn chat(provider: LlmProvider, request: &LlmRequest) -> Value {
    let mut body = Map::new();
    body.insert(
        "model".to_owned(),
        provider_model_id(provider, &request.model).into(),
    );
    let cap = match provider {
        LlmProvider::OpenAi => "max_completion_tokens",
        LlmProvider::Anthropic
        | LlmProvider::OpenRouter
        | LlmProvider::Together
        | LlmProvider::Nous => "max_tokens",
    };
    body.insert(cap.to_owned(), request.max_tokens.into());
    let system = request
        .system
        .iter()
        .map(|system| json!({ "role": "system", "content": system }));
    let messages = request
        .messages
        .iter()
        .map(|message| message_value(message, chat_part));
    body.insert("messages".to_owned(), system.chain(messages).collect());
    if let Output::Json { name, schema } = &request.output {
        body.insert(
            "response_format".to_owned(),
            json!({
                "type": "json_schema",
                "json_schema": { "name": name, "strict": true, "schema": schema }
            }),
        );
    }
    Value::Object(body)
}

fn message_value(message: &Message, part: fn(&Part) -> Value) -> Value {
    json!({
        "role": message.role.as_str(),
        "content": message.content.iter().map(part).collect::<Vec<_>>(),
    })
}

fn anthropic_part(part: &Part) -> Value {
    match part {
        Part::Text(text) => json!({ "type": "text", "text": text }),
        Part::Image {
            media_type,
            data_base64,
        } => json!({
            "type": "image",
            "source": { "type": "base64", "media_type": media_type, "data": data_base64 }
        }),
    }
}

fn chat_part(part: &Part) -> Value {
    match part {
        Part::Text(text) => json!({ "type": "text", "text": text }),
        Part::Image {
            media_type,
            data_base64,
        } => json!({
            "type": "image_url",
            "image_url": { "url": format!("data:{media_type};base64,{data_base64}") }
        }),
    }
}
