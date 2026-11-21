//! OpenAI chat/completions, also spoken by OpenRouter, Together and Nous.

use sc_agent_http::{Codec, http};
use sc_agent_types::Error;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{Chat, Message, Part, Reply, Role, Stop, Usage};

pub struct ChatCompletions {
    url: String,
    auth: Option<String>,
    /// OpenAI renamed `max_tokens`; the resellers did not.
    openai: bool,
}

impl ChatCompletions {
    pub fn new(base: String, key: Option<String>, openai: bool) -> Self {
        Self {
            url: format!("{base}/chat/completions"),
            auth: key.map(|k| format!("Bearer {k}")),
            openai,
        }
    }
}

/// One chat message, or one `tool` message per tool result.
fn messages(message: &Message) -> Vec<Value> {
    let mut content = Vec::new();
    let mut calls = Vec::new();
    let mut results = Vec::new();
    for part in &message.content {
        match part {
            Part::Text { text } => content.push(json!({ "type": "text", "text": text })),
            Part::Image { media_type, data } => {
                content.push(json!({ "type": "image_url", "image_url": { "url": format!("data:{media_type};base64,{data}") } }));
            }
            Part::ToolUse { id, name, input } => calls.push(json!({
                "id": id, "type": "function", "function": { "name": name, "arguments": input.to_string() }
            })),
            Part::ToolResult { id, content, .. } => {
                results.push(json!({ "role": "tool", "tool_call_id": id, "content": content }));
            }
        }
    }
    let mut out = Vec::new();
    if !content.is_empty() || !calls.is_empty() {
        let mut m = Map::new();
        m.insert("role".into(), json!(message.role));
        m.insert("content".into(), content.into());
        if message.role == Role::Assistant && !calls.is_empty() {
            m.insert("tool_calls".into(), calls.into());
        }
        out.push(Value::Object(m));
    }
    out.extend(results);
    out
}

#[derive(Deserialize)]
struct Call {
    id: String,
    function: Function,
}

#[derive(Deserialize)]
struct Function {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct Said {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<Call>,
}

#[derive(Deserialize)]
struct Choice {
    message: Said,
    finish_reason: Option<String>,
}

#[derive(Deserialize, Default)]
struct Tokens {
    prompt_tokens: u64,
    completion_tokens: u64,
}

#[derive(Deserialize)]
struct Envelope {
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Tokens,
}

impl Codec for ChatCompletions {
    type Request = Chat;
    type Response = Reply;

    fn encode(&self, chat: &Chat) -> Result<http::Request<Vec<u8>>, Error> {
        let mut body = Map::new();
        body.insert("model".into(), chat.model.clone().into());
        let limit = if self.openai {
            "max_completion_tokens"
        } else {
            "max_tokens"
        };
        body.insert(limit.into(), chat.max_tokens.into());
        let system = chat
            .system
            .iter()
            .map(|s| json!({ "role": "system", "content": s }));
        body.insert(
            "messages".into(),
            system
                .chain(chat.messages.iter().flat_map(messages))
                .collect(),
        );
        if !chat.tools.is_empty() {
            let tools = chat.tools.iter().map(|t| {
                json!({ "type": "function", "function": {
                "name": t.name, "description": t.description, "parameters": t.input_schema } })
            });
            body.insert("tools".into(), tools.collect());
        }
        if let Some(schema) = &chat.output_schema {
            body.insert(
                "response_format".into(),
                json!({ "type": "json_schema",
                "json_schema": { "name": "answer", "strict": true, "schema": schema } }),
            );
        }
        let auth = self.auth.as_deref().map(|auth| ("authorization", auth));
        sc_agent_http::post_json(&self.url, auth, &[], &Value::Object(body))
    }

    fn decode(&self, _chat: &Chat, status: u16, body: &[u8]) -> Result<Reply, Error> {
        let reply: Envelope = sc_agent_http::json_reply(status, body)?;
        let choice = reply
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| Error::ResponseParse("no choices".into()))?;
        let mut content: Vec<Part> = choice
            .message
            .content
            .into_iter()
            .map(|text| Part::Text { text })
            .collect();
        for call in choice.message.tool_calls {
            let input = serde_json::from_str(&call.function.arguments)
                .map_err(|e| Error::ResponseParse(e.to_string()))?;
            content.push(Part::ToolUse {
                id: call.id,
                name: call.function.name,
                input,
            });
        }
        let stop = match choice.finish_reason.as_deref() {
            Some("stop") => Stop::EndTurn,
            Some("tool_calls") => Stop::ToolUse,
            Some("length") => Stop::MaxTokens,
            other => Stop::Other(other.unwrap_or_default().to_owned()),
        };
        let usage = Usage {
            input_tokens: reply.usage.prompt_tokens,
            output_tokens: reply.usage.completion_tokens,
        };
        Ok(Reply {
            content,
            stop,
            usage,
        })
    }
}
