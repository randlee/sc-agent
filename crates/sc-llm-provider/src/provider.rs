//! [`LlmProvider`]: the providers that have an LLM wire.

use std::fmt;

use sc_agent_types::Provider;

/// A provider with an LLM wire: every [`Provider`] except TypeSafe.
///
/// TypeSafe serves Jev over the System One API and has no LLM wire, so it cannot be named
/// here; the caller converts a config [`Provider`] once with [`Self::from_provider`].
///
/// # Examples
///
/// ```
/// use sc_agent_types::Provider;
/// use sc_llm_provider::LlmProvider;
///
/// assert_eq!(LlmProvider::from_provider(Provider::OpenAi), Some(LlmProvider::OpenAi));
/// assert_eq!(LlmProvider::from_provider(Provider::TypeSafe), None);
/// assert_eq!(Provider::from(LlmProvider::Nous), Provider::Nous);
/// assert_eq!(LlmProvider::OpenAi.to_string(), "openai");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LlmProvider {
    /// Anthropic (Anthropic Messages).
    Anthropic,
    /// OpenAI (Chat Completions).
    OpenAi,
    /// OpenRouter (Chat Completions).
    OpenRouter,
    /// Together AI (Chat Completions).
    Together,
    /// Nous Research (Chat Completions).
    Nous,
}

impl LlmProvider {
    /// Every LLM provider, in declaration order.
    pub const ALL: &'static [Self] = &[
        Self::Anthropic,
        Self::OpenAi,
        Self::OpenRouter,
        Self::Together,
        Self::Nous,
    ];

    /// The LLM provider of `provider`; `None` for [`Provider::TypeSafe`], which has no LLM wire.
    #[must_use]
    pub const fn from_provider(provider: Provider) -> Option<Self> {
        match provider {
            Provider::TypeSafe => None,
            Provider::Anthropic => Some(Self::Anthropic),
            Provider::OpenAi => Some(Self::OpenAi),
            Provider::OpenRouter => Some(Self::OpenRouter),
            Provider::Together => Some(Self::Together),
            Provider::Nous => Some(Self::Nous),
        }
    }

    /// The documented spelling, the same as [`Provider::as_str`] (`openai`, not `open_ai`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        Provider::as_str(self.provider())
    }

    /// The [`Provider`] this is.
    const fn provider(self) -> Provider {
        match self {
            Self::Anthropic => Provider::Anthropic,
            Self::OpenAi => Provider::OpenAi,
            Self::OpenRouter => Provider::OpenRouter,
            Self::Together => Provider::Together,
            Self::Nous => Provider::Nous,
        }
    }
}

impl From<LlmProvider> for Provider {
    fn from(provider: LlmProvider) -> Self {
        provider.provider()
    }
}

impl fmt::Display for LlmProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
