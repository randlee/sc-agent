//! Anthropic Messages.

use sc_agent_types::Error;
use sc_agent_http::{Codec, http};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{Chat, Part, Reply, Stop, Usage};

const VERSION: &str = "2026-10-21";

pub struct Messages {
    url: String,
    key: Option<String>,
}

impl Messages {
    pub fn new(base: String, key: Option<String>) -> Self {
        Self { url: format!("{base}/messages"), key }
    }
}

fn part(part: &Part) -> Value {
    match part {
        Part::Text { text } => json!({ "type": "text", "text": text }),
        Part::Image { media_type, data } => {
            json!({ "type": "image", "source": { "type": "base64", "media_type": media_type, "data": data } })
        }
        Part::ToolUse { id, name, input } => json!({ "type": "tool_use", "id": id, "name": name, "input": input }),
        Part::ToolResult { id, content, is_error } => {
            json!({ "type": "tool_result", "tool_use_id": id, "content": content, "is_error": is_error })
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Block {
    Text { text: String },
    ToolUse { id: String, name: String, input: Value },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct Envelope {
    content: Vec<Block>,
    stop_reason: Option<String>,
    #[serde(default)]
    usage: Usage,
}

impl Codec for Messages {
    type Request = Chat;
    type Response = Reply;

    fn encode(&self, chat: &Chat) -> Result<http::Request<Vec<u8>>, Error> {
        let mut body = Map::new();
        body.insert("model".into(), chat.model.clone().into());
        body.insert("max_tokens".into(), chat.max_tokens.into());
        if let Some(system) = &chat.system {
            body.insert("system".into(), system.clone().into());
        }
        let messages = chat.messages.iter().map(|m| json!({ "role": m.role, "content": m.content.iter().map(part).collect::<Vec<_>>() }));
        body.insert("messages".into(), messages.collect());
        if !chat.tools.is_empty() {
            body.insert("tools".into(), chat.tools.iter().map(crate::tool_value).collect());
        }
        if let Some(schema) = &chat.output_schema {
            body.insert("output_config".into(), json!({ "format": { "type": "json_schema", "schema": schema } }));
        }
        let auth = self.key.as_deref().map(|key| ("x-api-key", key));
        sc_agent_http::post_json(&self.url, auth, &[("anthropic-version", VERSION)], &Value::Object(body))
    }

    fn decode(&self, _chat: &Chat, status: u16, body: &[u8]) -> Result<Reply, Error> {
        let reply: Envelope = sc_agent_http::json_reply(status, body)?;
        let content = reply
            .content
            .into_iter()
            .filter_map(|block| match block {
                Block::Text { text } => Some(Part::Text { text }),
                Block::ToolUse { id, name, input } => Some(Part::ToolUse { id, name, input }),
                Block::Other => None,
            })
            .collect();
        let stop = match reply.stop_reason.as_deref() {
            Some("end_turn") => Stop::EndTurn,
            Some("tool_use") => Stop::ToolUse,
            Some("max_tokens") => Stop::MaxTokens,
            other => Stop::Other(other.unwrap_or_default().to_owned()),
        };
        Ok(Reply { content, stop, usage: reply.usage })
    }
}
