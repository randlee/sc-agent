//! OpenAI chat/completions, also spoken by OpenRouter, Together and Nous.

use sc_agent_http::{Codec, http};
use sc_agent_types::Error;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{Chat, Message, Part, Reply, Usage};

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

/// One chat message.
fn messages(message: &Message) -> Value {
    let mut content = Vec::new();
    for part in &message.content {
        match part {
            Part::Text { text } => content.push(json!({ "type": "text", "text": text })),
            Part::Image { media_type, data } => {
                content.push(json!({ "type": "image_url", "image_url": { "url": format!("data:{media_type};base64,{data}") } }));
            }
        }
    }
    json!({ "role": message.role, "content": content })
}

#[derive(Deserialize)]
struct Said {
    content: Option<String>,
}

#[derive(Deserialize)]
struct Choice {
    message: Said,
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
            system.chain(chat.messages.iter().map(messages)).collect(),
        );
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
        let content: Vec<Part> = choice
            .message
            .content
            .into_iter()
            .map(|text| Part::Text { text })
            .collect();
        let usage = Usage {
            input_tokens: reply.usage.prompt_tokens,
            output_tokens: reply.usage.completion_tokens,
        };
        Ok(Reply { content, usage })
    }
}
