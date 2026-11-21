//! [`ShortName`], the nine model lines a caller can name.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::TypesError;

/// A model line, named the way a caller types it (`haiku`, `luna`, ...).
///
/// The documented spelling is the [`Display`](fmt::Display) form. Serde writes that form and
/// reads it through [`FromStr`]; there is no other spelling.
///
/// # Examples
///
/// ```
/// use sc_agent_types::ShortName;
///
/// let line: ShortName = "haiku".parse().unwrap();
/// assert_eq!(line, ShortName::Haiku);
/// assert_eq!(line.to_string(), "haiku");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ShortName {
    /// TypeSafe Jev.
    Jev,
    /// Claude Haiku.
    Haiku,
    /// Claude Sonnet.
    Sonnet,
    /// Claude Opus.
    Opus,
    /// Claude Fable.
    Fable,
    /// GPT Luna.
    Luna,
    /// GPT Terra.
    Terra,
    /// GPT Sol.
    Sol,
    /// GPT Astra.
    Astra,
}

impl ShortName {
    /// Every short name, in declaration order.
    pub const ALL: &'static [Self] = &[
        Self::Jev,
        Self::Haiku,
        Self::Sonnet,
        Self::Opus,
        Self::Fable,
        Self::Luna,
        Self::Terra,
        Self::Sol,
        Self::Astra,
    ];

    /// The documented spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Jev => "jev",
            Self::Haiku => "haiku",
            Self::Sonnet => "sonnet",
            Self::Opus => "opus",
            Self::Fable => "fable",
            Self::Luna => "luna",
            Self::Terra => "terra",
            Self::Sol => "sol",
            Self::Astra => "astra",
        }
    }
}

impl fmt::Display for ShortName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ShortName {
    type Err = TypesError;

    /// Parses the documented spelling; any other spelling is rejected.
    ///
    /// # Errors
    ///
    /// [`TypesError::UnknownShortName`] for anything but the nine spellings.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .find(|name| name.as_str() == text)
            .copied()
            .ok_or_else(|| TypesError::UnknownShortName {
                text: text.to_owned(),
            })
    }
}

impl Serialize for ShortName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ShortName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}
