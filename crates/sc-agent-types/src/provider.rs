/// Backend credential id consumed by `sc-agent-secrets`.
///
/// The documented spelling is the [`Display`](fmt::Display) form
/// (`typesafe`, `anthropic`, `openai`). Serde writes that form and reads it
/// through [`FromStr`]; there is no other spelling (bead `agt-ho2` removed
/// the `open_ai` read alias).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum BackendKeyId {
    /// TypeSafe Jev API credential.
    TypeSafe,
    /// Anthropic API credential.
    Anthropic,
    /// OpenAI API credential.
    OpenAi,
}

impl BackendKeyId {
    /// Every credential id, in declaration order.
    pub const ALL: &'static [Self] = &[Self::TypeSafe, Self::Anthropic, Self::OpenAi];

    /// The documented spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TypeSafe => "typesafe",
            Self::Anthropic => "anthropic",
            Self::OpenAi => "openai",
        }
    }

    /// The environment variable (and `.env` key) that supplies this credential.
    #[must_use]
    pub const fn env_var(self) -> &'static str {
        match self {
            Self::TypeSafe => "TYPESAFE_API_KEY",
            Self::Anthropic => "ANTHROPIC_API_KEY",
            Self::OpenAi => "OPENAI_API_KEY",
        }
    }
}

impl fmt::Display for BackendKeyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for BackendKeyId {
    type Err = JevError;

    /// Parses the documented id. Kebab-case and other spellings are rejected.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .find(|id| id.as_str() == text)
            .copied()
            .ok_or_else(|| {
                JevError::input_invalid(format!(
                    "backend key {text} is not typesafe, anthropic, or openai"
                ))
            })
    }
}

impl Serialize for BackendKeyId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for BackendKeyId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}
