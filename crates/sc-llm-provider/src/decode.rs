/// Parses a vendor reply. Error details carry no provider name; the caller
/// labels them.
pub(crate) fn parse_vendor(kind: ProviderKind, body: &[u8]) -> Result<VendorReply, JevError> {
    let value: Value = serde_json::from_slice(body).map_err(|_| {
        JevError::response_invalid_because(ResponseInvalidCause::NotJson, "reply is not JSON")
    })?;
    let object = value.as_object().ok_or_else(|| {
        JevError::response_invalid_because(ResponseInvalidCause::NotJson, "reply is not JSON")
    })?;
    let usage = object.get("usage").and_then(Value::as_object);
    let (text, non_text_blocks, input_tokens) = match kind {
        ProviderKind::Anthropic => parse_anthropic(object, usage)?,
        ProviderKind::OpenAi => parse_openai(object, usage)?,
    };
    let stop_reason = object
        .get("stop_reason")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let status = object
        .get("status")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let truncation = truncation_reason(object, stop_reason.as_deref(), status.as_deref());
    if text.is_none() && truncation.is_none() {
        return Err(JevError::response_invalid_because(
            ResponseInvalidCause::NotJson,
            "reply has no text",
        ));
    }
    Ok(VendorReply {
        text: text.unwrap_or_default(),
        input_tokens,
        non_text_blocks,
        stop_reason,
        status,
        truncation,
    })
}
fn parse_anthropic(
    object: &Map<String, Value>,
    usage: Option<&Map<String, Value>>,
) -> Result<(Option<String>, u64, u64), JevError> {
    let mut text = String::new();
    let mut non_text_blocks = 0_u64;
    let mut saw_text = false;
    for block in object
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(block) = block.as_object() else {
            continue;
        };
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(piece) = block.get("text").and_then(Value::as_str) {
                    text.push_str(piece);
                    saw_text = true;
                }
            }
            Some(kind) if kind.ends_with("tool_use") => {
                non_text_blocks = non_text_blocks.saturating_add(1)
            }
            _ => {}
        }
    }
    let input_tokens = usage_i64(usage, "input_tokens")
        .saturating_add(usage_i64(usage, "cache_creation_input_tokens"))
        .saturating_add(usage_i64(usage, "cache_read_input_tokens"));
    Ok((
        saw_text.then_some(text),
        non_text_blocks,
        nonneg(input_tokens)?,
    ))
}
fn truncation_reason(
    object: &Map<String, Value>,
    stop_reason: Option<&str>,
    status: Option<&str>,
) -> Option<String> {
    if stop_reason == Some("max_tokens") {
        return Some("reply truncated at max_tokens (stop_reason=max_tokens)".to_owned());
    }
    if let Some(status) =
        status.filter(|status| matches!(*status, "incomplete" | "max_tokens" | "max_output_tokens"))
    {
        return Some(format!("reply truncated at max_tokens (status={status})"));
    }
    let incomplete = object
        .get("incomplete_details")
        .and_then(|value| value.get("reason"))
        .and_then(Value::as_str);
    if incomplete == Some("max_output_tokens") {
        return Some(
            "reply truncated at max_tokens (incomplete_details.reason=max_output_tokens)"
                .to_owned(),
        );
    }
    None
}

/// A usage counter; a provider that omits one reports zero tokens for it.
fn usage_i64(usage: Option<&Map<String, Value>>, key: &str) -> i64 {
    usage
        .and_then(|object| object.get(key))
        .and_then(Value::as_i64)
        .unwrap_or_default()
}

fn nonneg(value: i64) -> Result<u64, JevError> {
    u64::try_from(value)
        .map_err(|_| JevError::response_invalid("provider input_tokens is negative"))
}
