
/// Caller text that labels a project in the session log.
///
/// UTF-8, at most [`Self::MAX_BYTES`] bytes, no control characters. The text
/// is caller text, not a secret, so `Debug` prints it. The CLI, Python
/// inputs and the session-log loader share this one validator.
///
/// # Examples
///
/// ```
/// use sc_agent_types::ProjectLabel;
///
/// let label: ProjectLabel = "billing-service".parse()?;
/// assert_eq!(label.as_str(), "billing-service");
/// assert_eq!(ProjectLabel::new("a\nb").unwrap_err().code(), "INPUT_INVALID");
/// # Ok::<(), sc_agent_types::JevError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectLabel(Arc<str>);

impl ProjectLabel {
    /// Longest accepted label, in UTF-8 bytes.
    pub const MAX_BYTES: usize = 256;

    /// Validates `text`.
    ///
    /// # Errors
    ///
    /// `INPUT_INVALID` when the text is longer than 256 bytes or has a
    /// control character; the fix hint states both limits.
    pub fn new(text: impl Into<String>) -> Result<Self, JevError> {
        let text = text.into();
        if text.len() > Self::MAX_BYTES || text.chars().any(char::is_control) {
            return Err(JevError::input_invalid(format!(
                "project label must be at most {} bytes of UTF-8 with no control characters (got {} bytes)",
                Self::MAX_BYTES,
                text.len()
            )));
        }
        Ok(Self(Arc::from(text)))
    }

    /// The label text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProjectLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ProjectLabel {
    type Err = JevError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::new(text)
    }
}

impl AsRef<str> for ProjectLabel {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Serialize for ProjectLabel {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ProjectLabel {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

