//! The request and response of any LLM call (REQ-SAG-0502), not only decisions.

use sc_agent_types::ModelId;

/// One LLM call: model, optional system prompt, messages, token cap and output form.
///
/// The model is the provider-neutral catalog id; [`LlmClient::encode`](crate::LlmClient::encode)
/// sends [`provider_model_id`](crate::provider_model_id) of it.
#[derive(Debug, Clone, PartialEq)]
pub struct LlmRequest {
    /// The catalog model id.
    pub model: ModelId,
    /// The system prompt, if any.
    pub system: Option<String>,
    /// The conversation, in order.
    pub messages: Vec<Message>,
    /// The completion token cap (`max_tokens`, or `max_completion_tokens` for OpenAI).
    pub max_tokens: u32,
    /// Plain text or a JSON-schema reply.
    pub output: Output,
}

/// One message of the conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Who wrote it.
    pub role: Role,
    /// Its parts, in order.
    pub content: Vec<Part>,
}

/// One part of a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    /// Text.
    Text(String),
    /// An image, base64-encoded.
    ///
    /// Anthropic `{type:"image", source:{type:"base64", media_type, data}}`; Chat
    /// `{type:"image_url", image_url:{url:"data:<media_type>;base64,<data>"}}`.
    Image {
        /// The MIME type, e.g. `image/png`.
        media_type: &'static str,
        /// The image bytes, base64.
        data_base64: String,
    },
}

/// The author of a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// The caller.
    User,
    /// The model.
    Assistant,
}

impl Role {
    /// The wire spelling (`user`, `assistant`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

/// The form of the reply.
#[derive(Debug, Clone, PartialEq)]
pub enum Output {
    /// Free text.
    Text,
    /// A JSON value matching `schema`, through the provider's structured-output feature.
    ///
    /// Anthropic `output_config.format = {type:"json_schema", schema}`;
    /// Chat Completions `response_format = {type:"json_schema", json_schema:{name, strict:true, schema}}`.
    Json {
        /// The schema name (Chat Completions `json_schema.name`).
        name: String,
        /// The JSON schema the reply must match.
        schema: serde_json::Value,
    },
}

/// A decoded final reply.
#[derive(Debug, Clone, PartialEq)]
pub struct LlmResponse {
    /// The model as the provider reports it.
    pub model: String,
    /// The reply text.
    pub text: String,
    /// The parsed text, for [`Output::Json`]; `None` for [`Output::Text`].
    pub json: Option<serde_json::Value>,
    /// The provider's stop reason (`stop_reason` or `finish_reason`), verbatim.
    pub stop_reason: Option<String>,
    /// The provider's token counters.
    pub usage: Usage,
}

/// Token counters of one reply, as the provider wire reports them.
///
/// Maps Anthropic `usage.input_tokens/output_tokens` and Chat Completions
/// `usage.prompt_tokens/completion_tokens`; an omitted counter is 0. Not a Jev-API-mapped type
/// (the Jev usage is `sc_decision_schema::Usage`), so it is outside REQ-SAG-0201 AC2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage {
    /// Input (prompt) tokens.
    pub input_tokens: u64,
    /// Output (completion) tokens.
    pub output_tokens: u64,
}
