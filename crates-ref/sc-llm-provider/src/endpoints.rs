//! Built-in endpoints and the headers of each wire.

use sc_agent_http::SecretHeader;
use sc_agent_secrets::ApiKey;

use crate::{LlmProvider, Wire};

const ANTHROPIC_HOST: &str = "api.anthropic.com";
const ANTHROPIC_MESSAGE_PATH: &str = "/v1/messages";

/// Anthropic REST version sent on every Anthropic call.
///
/// Pinned here: a different version is a different wire contract.
pub(crate) const ANTHROPIC_VERSION: &str = "2026-10-21";

/// The Chat Completions path, appended to each OpenAI-compatible base.
const CHAT_PATH: &str = "/chat/completions";
/// The models-list path, appended to the same base.
const MODELS_PATH: &str = "/models";

/// The base of a provider's paths (`{base}/chat/completions`,
/// `{base}/models`).
const fn chat_base(provider: LlmProvider) -> &'static str {
    match provider {
        LlmProvider::OpenAi => "https://api.openai.com/v1",
        LlmProvider::OpenRouter => "https://openrouter.ai/api/v1",
        LlmProvider::Together => "https://api.together.ai/v1",
        LlmProvider::Nous => "https://inference-api.nousresearch.com/v1",
        LlmProvider::Anthropic => "https://api.anthropic.com/v1",
    }
}

/// The suffix of the call URL of `wire`, which [`models_url`] replaces with `/models`.
pub(crate) const fn call_suffix(wire: Wire) -> &'static str {
    match wire {
        Wire::AnthropicMessages => "/messages",
        Wire::ChatCompletions => CHAT_PATH,
    }
}

/// The built-in call URL of `provider`.
pub(crate) fn call_url(provider: LlmProvider) -> String {
    match Wire::of(provider) {
        Wire::AnthropicMessages => format!("https://{ANTHROPIC_HOST}{ANTHROPIC_MESSAGE_PATH}"),
        Wire::ChatCompletions => format!("{}{CHAT_PATH}", chat_base(provider)),
    }
}

/// The models-list URL beside the call URL `call`: its `call_suffix` replaced with `/models`
/// (`https://api.anthropic.com/v1/messages` -> `https://api.anthropic.com/v1/models`). A call
/// URL without the suffix (a `with_endpoint` override) gets `/models` appended.
pub(crate) fn models_url(call: &str, wire: Wire) -> String {
    let stem = call.strip_suffix(call_suffix(wire)).unwrap_or(call);
    format!("{}{MODELS_PATH}", stem.trim_end_matches('/'))
}

/// Provider headers that do not carry the API key.
///
/// The credential goes separately, as [`secret_header`]; only the sending transport writes
/// its value.
pub(crate) fn unsigned_headers(wire: Wire) -> Vec<(&'static str, &'static str)> {
    let mut headers = match wire {
        Wire::AnthropicMessages => vec![(ANTHROPIC_VERSION_HEADER, ANTHROPIC_VERSION)],
        Wire::ChatCompletions => Vec::new(),
    };
    headers.push((CONTENT_TYPE, "application/json"));
    headers
}

/// The credential header each wire expects: Anthropic `x-api-key`, Chat Completions
/// `Authorization: Bearer`.
pub(crate) fn secret_header(wire: Wire, key: ApiKey) -> SecretHeader {
    match wire {
        Wire::AnthropicMessages => SecretHeader::new(X_API_KEY, key),
        Wire::ChatCompletions => SecretHeader::bearer(key),
    }
}

const ANTHROPIC_VERSION_HEADER: &str = "anthropic-version";
const CONTENT_TYPE: &str = "content-type";
const X_API_KEY: &str = "x-api-key";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_urls() {
        assert_eq!(
            call_url(LlmProvider::Anthropic),
            format!("https://{ANTHROPIC_HOST}{ANTHROPIC_MESSAGE_PATH}")
        );
        assert_eq!(
            call_url(LlmProvider::OpenAi),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            call_url(LlmProvider::OpenRouter),
            "https://openrouter.ai/api/v1/chat/completions"
        );
        assert_eq!(
            call_url(LlmProvider::Together),
            "https://api.together.ai/v1/chat/completions"
        );
        assert_eq!(
            call_url(LlmProvider::Nous),
            "https://inference-api.nousresearch.com/v1/chat/completions"
        );
        assert_eq!(
            models_url(&call_url(LlmProvider::Anthropic), Wire::AnthropicMessages),
            "https://api.anthropic.com/v1/models"
        );
        assert_eq!(
            models_url(&call_url(LlmProvider::OpenRouter), Wire::ChatCompletions),
            "https://openrouter.ai/api/v1/models"
        );
        assert_eq!(
            models_url("https://gateway.test/v1/", Wire::ChatCompletions),
            "https://gateway.test/v1/models"
        );
    }
}
