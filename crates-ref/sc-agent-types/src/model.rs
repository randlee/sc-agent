//! [`ModelId`], the built-in model rows ([`MODELS`]) and their descriptor.

use std::borrow::{Borrow, Cow};
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{ReleaseDate, ShortName, TypesError};

/// A provider-neutral model id, such as `claude-haiku-5-5` (REQ-SAG-0103).
///
/// It is non-empty and has no control characters. It names no provider: each provider maps
/// it to its own id. The field is private; the ways to get one are [`FromStr`], serde and the
/// built-in [`MODELS`] rows.
///
/// # Examples
///
/// ```
/// use sc_agent_types::ModelId;
///
/// let id: ModelId = "claude-haiku-5-5".parse().unwrap();
/// assert_eq!(id.as_str(), "claude-haiku-5-5");
/// assert!("".parse::<ModelId>().is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModelId(Cow<'static, str>);

impl ModelId {
    /// The model id text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// A model id from a literal the crate knows is valid (the built-in rows).
    pub(crate) const fn from_static(text: &'static str) -> Self {
        Self(Cow::Borrowed(text))
    }
}

impl fmt::Display for ModelId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ModelId {
    type Err = TypesError;

    /// Parses any non-empty text with no control characters; the catalog is not consulted.
    ///
    /// # Errors
    ///
    /// [`TypesError::InvalidModelId`] when the text is empty or has a control character.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.is_empty() || text.chars().any(char::is_control) {
            return Err(TypesError::InvalidModelId {
                text: text.to_owned(),
            });
        }
        Ok(Self(Cow::Owned(text.to_owned())))
    }
}

impl AsRef<str> for ModelId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

// The derived Eq, Ord and Hash on `Cow<str>` agree with those of `str`, as `Borrow` requires.
impl Borrow<str> for ModelId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Serialize for ModelId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ModelId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// One row of [`MODELS`]: a model id, the line it belongs to and its published release date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelDescriptor {
    model: ModelId,
    line: ShortName,
    released: ReleaseDate,
}

impl ModelDescriptor {
    /// The model of this row.
    #[must_use]
    pub const fn model(&self) -> &ModelId {
        &self.model
    }

    /// The line this row belongs to.
    #[must_use]
    pub const fn line(&self) -> ShortName {
        self.line
    }

    /// The model's published release date.
    #[must_use]
    pub const fn released(&self) -> ReleaseDate {
        self.released
    }
}

// One const per built-in row, so the catalog can seed every line from a row of its own.
// Dates are the published release dates (research/model-dates.md, checked 2026-11-09);
// jev-1.13.0's date was supplied by the owner (docs.typesafe.ai publishes none).
pub(crate) const JEV_1_13_0: ModelDescriptor = row(
    ModelId::from_static("jev-1.13.0"),
    ShortName::Jev,
    ReleaseDate::ymd(2026, 9, 15),
);
pub(crate) const JEV_1_14_0: ModelDescriptor = row(
    ModelId::from_static("jev-1.14.0"),
    ShortName::Jev,
    ReleaseDate::ymd(2026, 10, 14),
);
pub(crate) const CLAUDE_HAIKU_5_5: ModelDescriptor = row(
    ModelId::from_static("claude-haiku-5-5"),
    ShortName::Haiku,
    ReleaseDate::ymd(2026, 10, 7),
);
pub(crate) const CLAUDE_SONNET_5_5: ModelDescriptor = row(
    ModelId::from_static("claude-sonnet-5-5"),
    ShortName::Sonnet,
    ReleaseDate::ymd(2026, 9, 28),
);
pub(crate) const CLAUDE_OPUS_5_5: ModelDescriptor = row(
    ModelId::from_static("claude-opus-5-5"),
    ShortName::Opus,
    ReleaseDate::ymd(2026, 9, 22),
);
pub(crate) const CLAUDE_FABLE_5_2: ModelDescriptor = row(
    ModelId::from_static("claude-fable-5-2"),
    ShortName::Fable,
    ReleaseDate::ymd(2026, 11, 9),
);
pub(crate) const GPT_6_LUNA: ModelDescriptor = row(
    ModelId::from_static("gpt-6-luna"),
    ShortName::Luna,
    ReleaseDate::ymd(2026, 9, 22),
);
pub(crate) const GPT_6_1_TERRA: ModelDescriptor = row(
    ModelId::from_static("gpt-6.1-terra"),
    ShortName::Terra,
    ReleaseDate::ymd(2026, 10, 26),
);
pub(crate) const GPT_6_1_SOL: ModelDescriptor = row(
    ModelId::from_static("gpt-6.1-sol"),
    ShortName::Sol,
    ReleaseDate::ymd(2026, 9, 29),
);
pub(crate) const GPT_6_ASTRA: ModelDescriptor = row(
    ModelId::from_static("gpt-6-astra"),
    ShortName::Astra,
    ReleaseDate::ymd(2026, 9, 3),
);

/// The built-in models. Per line, the newest `released` row is the default.
///
/// Models released after this build are added through the global config
/// ([`Catalog::with_config_model`](crate::Catalog::with_config_model)), with no rebuild.
pub static MODELS: &[ModelDescriptor] = &[
    JEV_1_13_0,
    JEV_1_14_0,
    CLAUDE_HAIKU_5_5,
    CLAUDE_SONNET_5_5,
    CLAUDE_OPUS_5_5,
    CLAUDE_FABLE_5_2,
    GPT_6_LUNA,
    GPT_6_1_TERRA,
    GPT_6_1_SOL,
    GPT_6_ASTRA,
];

const fn row(model: ModelId, line: ShortName, released: ReleaseDate) -> ModelDescriptor {
    ModelDescriptor {
        model,
        line,
        released,
    }
}
