#![forbid(unsafe_code)]
//! Chat with any LLM provider. Not decision-specific: a tool loop, a summarizer and the
//! decision adapter all use the same [`Chat`] request and the same [`sc_agent_http::Client`].

use sc_agent_types::Error;
use sc_agent_http::Codec;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

mod chat;
mod messages;

pub use chat::ChatCompletions;
pub use messages::Messages;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// One piece of a message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Part {
    Text { text: String },
    /// `data` is already base64.
    Image { media_type: String, data: String },
    ToolUse { id: String, name: String, input: Value },
    ToolResult { id: String, content: String, is_error: bool },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<Part>,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self { role: Role::User, content: vec![Part::Text { text: text.into() }] }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// A chat request, the same for every provider.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Chat {
    pub model: String,
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub tools: Vec<Tool>,
    pub max_tokens: u32,
    /// When set, the reply text is JSON matching this schema.
    pub output_schema: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stop {
    EndTurn,
    ToolUse,
    MaxTokens,
    Other(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reply {
    pub content: Vec<Part>,
    pub stop: Stop,
    pub usage: Usage,
}

impl Reply {
    pub fn text(&self) -> String {
        self.content.iter().filter_map(|p| if let Part::Text { text } = p { Some(text.as_str()) } else { None }).collect()
    }

    /// The assistant message to append to the conversation.
    pub fn message(&self) -> Message {
        Message { role: Role::Assistant, content: self.content.clone() }
    }
}

/// Any chat backend: a [`Codec`] from [`Chat`] to [`Reply`].
pub trait Llm: Codec<Request = Chat, Response = Reply> {}
impl<T: Codec<Request = Chat, Response = Reply> + ?Sized> Llm for T {}

/// The chat backend for a provider's wire, checked once.
pub fn backend(provider: &sc_agent_types::providers::Provider, key: Option<String>) -> Result<Box<dyn Llm>, Error> {
    let base = sc_agent_http::check_base(provider.base_url, key.is_some())?;
    Ok(match provider.wire {
        sc_agent_types::providers::Wire::Messages => Box::new(Messages::new(base, key)),
        sc_agent_types::providers::Wire::Chat => Box::new(ChatCompletions::new(base, key, provider.name == "openai")),
        sc_agent_types::providers::Wire::SystemOne => {
            return Err(Error::Config(format!("{} is a decision API, not a chat API", provider.name)));
        }
    })
}

fn tool_value(tool: &Tool) -> Value {
    json!({ "name": tool.name, "description": tool.description, "input_schema": tool.input_schema })
}
