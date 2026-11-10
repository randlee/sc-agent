//! Reply envelopes of the two wires.

use serde_json::{Map, Value};

use crate::{LlmError, LlmProvider, Usage, Wire};

/// What a 2xx reply envelope says, before the output is checked.
#[derive(Debug)]
pub(crate) struct Envelope {
    /// The model as the provider reports it.
    pub(crate) model: String,
    /// The reply text, when the reply has any text content.
    pub(crate) text: Option<String>,
    /// `stop_reason` (Anthropic) or `finish_reason` (Chat), verbatim.
    pub(crate) stop_reason: Option<String>,
    /// The refusal text, when the model refused (an empty string when it gave none).
    pub(crate) refusal: Option<String>,
    /// The reply stopped at the token cap.
    pub(crate) truncated: bool,
    /// The provider's counters.
    pub(crate) usage: Usage,
}

/// Parses a 2xx reply body of `wire`.
///
/// # Errors
///
/// [`LlmError::ReplyInvalid`] when the body is not JSON, not the wire's envelope, or a usage
/// counter is negative. The message is serde's or names the field, never body bytes.
pub(crate) fn decode_envelope(
    provider: LlmProvider,
    wire: Wire,
    body: &[u8],
) -> Result<Envelope, LlmError> {
    let invalid = |message: String| LlmError::ReplyInvalid { provider, message };
    let value: Value =
        serde_json::from_slice(body).map_err(|error| invalid(format!("not JSON: {error}")))?;
    let object = value
        .as_object()
        .ok_or_else(|| invalid("the reply is not a JSON object".to_owned()))?;
    let model = object
        .get("model")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("the reply has no `model` string".to_owned()))?
        .to_owned();
    let usage = object.get("usage").and_then(Value::as_object);
    match wire {
        Wire::AnthropicMessages => decode_anthropic(object, usage, model).map_err(invalid),
        Wire::ChatCompletions => decode_chat(object, usage, model).map_err(invalid),
    }
}

/// Anthropic Messages: text blocks concatenated, `stop_reason` `max_tokens` / `refusal`.
fn decode_anthropic(
    object: &Map<String, Value>,
    usage: Option<&Map<String, Value>>,
    model: String,
) -> Result<Envelope, String> {
    let blocks = object
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| "the reply has no `content` array".to_owned())?;
    let mut text = String::new();
    let mut saw_text = false;
    for block in blocks.iter().filter_map(Value::as_object) {
        if block.get("type").and_then(Value::as_str) == Some("text")
            && let Some(piece) = block.get("text").and_then(Value::as_str)
        {
            text.push_str(piece);
            saw_text = true;
        }
    }
    let stop_reason = object
        .get("stop_reason")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let refusal = (stop_reason.as_deref() == Some("refusal")).then(|| text.clone());
    Ok(Envelope {
        model,
        truncated: stop_reason.as_deref() == Some("max_tokens"),
        text: saw_text.then_some(text),
        stop_reason,
        refusal,
        usage: Usage {
            input_tokens: counter(usage, "input_tokens")?,
            output_tokens: counter(usage, "output_tokens")?,
        },
    })
}

/// Chat Completions: the first choice's `message.content`, `finish_reason` `length`,
/// `message.refusal` non-null.
fn decode_chat(
    object: &Map<String, Value>,
    usage: Option<&Map<String, Value>>,
    model: String,
) -> Result<Envelope, String> {
    let choice = object
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(Value::as_object)
        .ok_or_else(|| "the reply has no `choices[0]` object".to_owned())?;
    let message = choice
        .get("message")
        .and_then(Value::as_object)
        .ok_or_else(|| "the reply has no `choices[0].message` object".to_owned())?;
    let stop_reason = choice
        .get("finish_reason")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let refusal = match message.get("refusal") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => Some(text.clone()),
        Some(other) => Some(other.to_string()),
    };
    Ok(Envelope {
        model,
        truncated: stop_reason.as_deref() == Some("length"),
        text: message
            .get("content")
            .and_then(Value::as_str)
            .map(str::to_owned),
        stop_reason,
        refusal,
        usage: Usage {
            input_tokens: counter(usage, "prompt_tokens")?,
            output_tokens: counter(usage, "completion_tokens")?,
        },
    })
}

/// A usage counter; a provider that omits one reports zero tokens for it.
fn counter(usage: Option<&Map<String, Value>>, key: &str) -> Result<u64, String> {
    let Some(value) = usage.and_then(|object| object.get(key)) else {
        return Ok(0);
    };
    match value.as_i64() {
        Some(count) => {
            u64::try_from(count).map_err(|_| format!("usage `{key}` is negative ({count})"))
        }
        None => value
            .as_u64()
            .ok_or_else(|| format!("usage `{key}` is not an integer")),
    }
}

/// The model ids of a models-list body: `data[].id`, or a top-level array of `{id}`.
pub(crate) fn decode_model_ids(
    provider: LlmProvider,
    body: &[u8],
) -> Result<Vec<String>, LlmError> {
    let invalid = |message: String| LlmError::ReplyInvalid { provider, message };
    let value: Value =
        serde_json::from_slice(body).map_err(|error| invalid(format!("not JSON: {error}")))?;
    let entries = match &value {
        Value::Array(entries) => entries,
        Value::Object(object) => object
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("the models list has no `data` array".to_owned()))?,
        _ => {
            return Err(invalid(
                "the models list is not an object or array".to_owned(),
            ));
        }
    };
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            entry
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| invalid(format!("models entry {index} has no `id` string")))
        })
        .collect()
}
