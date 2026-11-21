//! Catalog model id <-> provider model id (REQ-SAG-0103).

use sc_agent_types::ModelId;

use crate::LlmProvider;

/// The model id `provider` expects for the catalog id `model`.
///
/// Anthropic, OpenAI and Nous use the catalog id unchanged. OpenRouter prefixes `claude-*` ids
/// with `anthropic/`, writing a trailing `-<major>-<minor>` as `<major>.<minor>`
/// (`claude-haiku-5-5` -> `anthropic/claude-haiku-5.5`), and `gpt-*` ids with `openai/`.
/// Together prefixes `claude-*` with `anthropic/` and `gpt-*` with `openai/`, id unchanged
/// (Together's ids are unconfirmed; decidr doctor reports any that do not map back). Any other
/// id is sent unchanged.
///
/// # Examples
///
/// ```
/// use sc_llm_provider::{LlmProvider, provider_model_id};
///
/// let model = "gpt-6.1-sol".parse()?;
/// assert_eq!(provider_model_id(LlmProvider::OpenAi, &model), "gpt-6.1-sol");
/// assert_eq!(provider_model_id(LlmProvider::OpenRouter, &model), "openai/gpt-6.1-sol");
/// # Ok::<(), sc_agent_types::TypesError>(())
/// ```
#[must_use]
pub fn provider_model_id(provider: LlmProvider, model: &ModelId) -> String {
    let id = model.as_str();
    match provider {
        LlmProvider::Anthropic | LlmProvider::OpenAi | LlmProvider::Nous => id.to_owned(),
        LlmProvider::OpenRouter => {
            if id.starts_with(CLAUDE) {
                format!("{ANTHROPIC_PREFIX}{}", dotted_version(id))
            } else if id.starts_with(GPT) {
                format!("{OPENAI_PREFIX}{id}")
            } else {
                id.to_owned()
            }
        }
        LlmProvider::Together => {
            if id.starts_with(CLAUDE) {
                format!("{ANTHROPIC_PREFIX}{id}")
            } else if id.starts_with(GPT) {
                format!("{OPENAI_PREFIX}{id}")
            } else {
                id.to_owned()
            }
        }
    }
}

/// The catalog id of the id `provider_id` that `provider` lists: the inverse of
/// [`provider_model_id`], used by decidr doctor.
///
/// An id that no rule produced is returned unchanged.
///
/// # Examples
///
/// ```
/// use sc_llm_provider::{LlmProvider, catalog_model_id};
///
/// assert_eq!(catalog_model_id(LlmProvider::OpenRouter, "anthropic/claude-haiku-5.5"), "claude-haiku-5-5");
/// assert_eq!(catalog_model_id(LlmProvider::Together, "openai/gpt-6-luna"), "gpt-6-luna");
/// ```
#[must_use]
pub fn catalog_model_id(provider: LlmProvider, provider_id: &str) -> String {
    match provider {
        LlmProvider::Anthropic | LlmProvider::OpenAi | LlmProvider::Nous => provider_id.to_owned(),
        LlmProvider::OpenRouter => {
            if let Some(id) = provider_id
                .strip_prefix(ANTHROPIC_PREFIX)
                .filter(|id| id.starts_with(CLAUDE))
            {
                dashed_version(id)
            } else if let Some(id) = provider_id
                .strip_prefix(OPENAI_PREFIX)
                .filter(|id| id.starts_with(GPT))
            {
                id.to_owned()
            } else {
                provider_id.to_owned()
            }
        }
        LlmProvider::Together => provider_id
            .strip_prefix(ANTHROPIC_PREFIX)
            .filter(|id| id.starts_with(CLAUDE))
            .or_else(|| {
                provider_id
                    .strip_prefix(OPENAI_PREFIX)
                    .filter(|id| id.starts_with(GPT))
            })
            .unwrap_or(provider_id)
            .to_owned(),
    }
}

const CLAUDE: &str = "claude-";
const GPT: &str = "gpt-";
const ANTHROPIC_PREFIX: &str = "anthropic/";
const OPENAI_PREFIX: &str = "openai/";

fn is_number(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// `<stem>-<major>-<minor>` -> `<stem>-<major>.<minor>`; any other id unchanged.
fn dotted_version(id: &str) -> String {
    let Some((head, minor)) = id.rsplit_once('-') else {
        return id.to_owned();
    };
    let Some((stem, major)) = head.rsplit_once('-') else {
        return id.to_owned();
    };
    if is_number(major) && is_number(minor) {
        format!("{stem}-{major}.{minor}")
    } else {
        id.to_owned()
    }
}

/// `<stem>-<major>.<minor>` -> `<stem>-<major>-<minor>`; any other id unchanged.
fn dashed_version(id: &str) -> String {
    let Some((stem, version)) = id.rsplit_once('-') else {
        return id.to_owned();
    };
    match version.split_once('.') {
        Some((major, minor)) if is_number(major) && is_number(minor) => {
            format!("{stem}-{major}-{minor}")
        }
        _ => id.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_rewrites_need_two_numbers() {
        assert_eq!(dotted_version("claude-haiku-5-5"), "claude-haiku-5.5");
        assert_eq!(dotted_version("claude-haiku-latest"), "claude-haiku-latest");
        assert_eq!(dotted_version("claude"), "claude");
        assert_eq!(dashed_version("claude-haiku-5.5"), "claude-haiku-5-5");
        assert_eq!(dashed_version("claude-haiku-5"), "claude-haiku-5");
        assert_eq!(dashed_version("claude-haiku-5.x"), "claude-haiku-5.x");
    }
}
