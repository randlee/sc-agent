use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::JevError;

/// A model string this build does not know.
///
/// The field is private and there is no public constructor. The only ways to
/// get one are [`ModelId::from_str`] (which returns the known variant for a
/// known string, so [`ModelId::Other`] never holds a known string and the
/// derived `PartialEq` and `Hash` stay correct) and `Deserialize`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnknownModel(Arc<str>);

impl UnknownModel {
    /// The model string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for ModelId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ModelId {
    type Err = JevError;

    /// Parses a model string: the known variant when the catalog has it,
    /// else [`ModelId::Other`].
    ///
    /// # Errors
    ///
    /// `INPUT_INVALID` when the text is empty or has a control character.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.is_empty() || text.chars().any(char::is_control) {
            return Err(JevError::input_invalid(format!(
                "model id {text:?} must be non-empty with no control characters; use a non-empty model string or an alias such as haiku"
            )));
        }
        Ok(Self::known(text).unwrap_or_else(|| Self::Other(UnknownModel(Arc::from(text)))))
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
/// One row of [`MODELS`].
#[derive(Debug, Clone, Copy)]
pub struct ModelDescriptor {
    model: &'static str,
    family: ModelFamily,
    alias: &'static str,
    released: &'static str,
}

impl ModelDescriptor {
    /// The model string of this row, without parsing it.
    #[must_use]
    pub const fn model_str(&self) -> &'static str {
        self.model
    }

    /// The model of this row.
    ///
    /// # Panics
    ///
    /// Never for a `MODELS` row: `test_known_models_roundtrip` proves every
    /// row string is a valid model string (M-PANIC-ON-BUG).
    #[must_use]
    pub fn model(&self) -> ModelId {
        self.model
            .parse()
            .expect("MODELS rows are valid model strings (test_known_models_roundtrip)")
    }

    /// The model family.
    #[must_use]
    pub const fn family(&self) -> ModelFamily {
        self.family
    }

    /// The alias this row answers to (`"jev"`, `"haiku"`, `"luna"`,
    /// `"sonnet"`, `"opus"`, `"fable"`, `"terra"`, `"sol"`, `"astra"`).
    #[must_use]
    pub const fn alias(&self) -> &'static str {
        self.alias
    }

    /// Release date, `YYYY-MM-DD`.
    #[must_use]
    pub const fn released(&self) -> &'static str {
        self.released
    }
}

/// The known models. Per alias, the newest `released` row is the default.
///
/// Only `jev`, `haiku` and `luna` are selectable as `--backend`; the other
/// aliases are definitions for future agent work (agt-akp). Models added
/// after this build go in `~/.sc-agent/models.toml` ([`ModelAliases::with_file_model`]);
/// a local freshness test tells maintainers when this table is behind
/// (randlee/sc-agent#48).
pub static MODELS: &[ModelDescriptor] = &[
    row("jev-1.13.0", ModelFamily::TypeSafeJev, "jev", "2026-06-01"),
    row("jev-1.14.0", ModelFamily::TypeSafeJev, "jev", "2026-10-22"),
    row(
        "claude-haiku-5-5",
        ModelFamily::AnthropicClaude,
        "haiku",
        "2026-09-01",
    ),
    row(
        "claude-sonnet-5-5",
        ModelFamily::AnthropicClaude,
        "sonnet",
        "2026-09-01",
    ),
    row(
        "claude-opus-5-5",
        ModelFamily::AnthropicClaude,
        "opus",
        "2026-09-01",
    ),
    row(
        "claude-fable-5-1",
        ModelFamily::AnthropicClaude,
        "fable",
        "2026-10-01",
    ),
    row("gpt-6-luna", ModelFamily::OpenAiGpt, "luna", "2026-08-01"),
    row(
        "gpt-6.1-terra",
        ModelFamily::OpenAiGpt,
        "terra",
        "2026-10-01",
    ),
    row("gpt-6.1-sol", ModelFamily::OpenAiGpt, "sol", "2026-10-01"),
    row("gpt-6-astra", ModelFamily::OpenAiGpt, "astra", "2026-08-01"),
];

const fn row(
    model: &'static str,
    family: ModelFamily,
    alias: &'static str,
    released: &'static str,
) -> ModelDescriptor {
    ModelDescriptor {
        model,
        family,
        alias,
        released,
    }
}
