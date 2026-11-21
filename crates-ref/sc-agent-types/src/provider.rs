//! [`Provider`], the six services that serve models, and their fixed ranking.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{ShortName, TypesError};

/// A service that serves models: a separate type from [`ModelId`](crate::ModelId), which
/// names no provider (REQ-SAG-0103).
///
/// The documented spelling is the [`Display`](fmt::Display) form (`typesafe`, `anthropic`,
/// `openai`, `openrouter`, `nous`, `together`). Serde writes that form and reads it through
/// [`FromStr`]; there is no other spelling. The enum is closed so that credential lookup can
/// match it exhaustively.
///
/// # Examples
///
/// ```
/// use sc_agent_types::{Provider, ShortName};
///
/// assert_eq!(Provider::OpenAi.to_string(), "openai");
/// assert_eq!(
///     Provider::ranking(Some(ShortName::Haiku)),
///     &[Provider::Anthropic, Provider::OpenRouter, Provider::Together]
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Provider {
    /// TypeSafe, which serves Jev.
    TypeSafe,
    /// Anthropic, which serves Claude.
    Anthropic,
    /// OpenAI, which serves GPT.
    OpenAi,
    /// OpenRouter, which resells many providers' models.
    OpenRouter,
    /// Nous Research.
    Nous,
    /// Together AI.
    Together,
}

impl Provider {
    /// Every provider, in declaration order.
    pub const ALL: &'static [Self] = &[
        Self::TypeSafe,
        Self::Anthropic,
        Self::OpenAi,
        Self::OpenRouter,
        Self::Nous,
        Self::Together,
    ];

    /// The fixed priority ranking (REQ-SAG-0103): TypeSafe, Anthropic, OpenAi, OpenRouter,
    /// Together, Nous.
    pub const PRIORITY: [Self; 6] = [
        Self::TypeSafe,
        Self::Anthropic,
        Self::OpenAi,
        Self::OpenRouter,
        Self::Together,
        Self::Nous,
    ];

    /// The providers that serve `line`, in [`Self::PRIORITY`] order.
    ///
    /// `None` stands for a model id on no line of the catalog, which only the resellers can
    /// be assumed to serve.
    #[must_use]
    pub const fn ranking(line: Option<ShortName>) -> &'static [Self] {
        match line {
            Some(ShortName::Jev) => &[Self::TypeSafe],
            Some(ShortName::Haiku | ShortName::Sonnet | ShortName::Opus | ShortName::Fable) => {
                &[Self::Anthropic, Self::OpenRouter, Self::Together]
            }
            Some(ShortName::Luna | ShortName::Terra | ShortName::Sol | ShortName::Astra) => {
                &[Self::OpenAi, Self::OpenRouter, Self::Together]
            }
            None => &[Self::OpenRouter, Self::Together, Self::Nous],
        }
    }

    /// The documented spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TypeSafe => "typesafe",
            Self::Anthropic => "anthropic",
            Self::OpenAi => "openai",
            Self::OpenRouter => "openrouter",
            Self::Nous => "nous",
            Self::Together => "together",
        }
    }
}

impl fmt::Display for Provider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Provider {
    type Err = TypesError;

    /// Parses the documented spelling; any other spelling is rejected.
    ///
    /// # Errors
    ///
    /// [`TypesError::UnknownProvider`] for anything but the six spellings.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .find(|provider| provider.as_str() == text)
            .copied()
            .ok_or_else(|| TypesError::UnknownProvider {
                text: text.to_owned(),
            })
    }
}

impl Serialize for Provider {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Provider {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}
