//! [`Slug`]: the validated text of `project` and `agent_role` (REQ-SAG-0004).

use std::fmt;
use std::sync::Arc;

use crate::LogError;
use crate::name::is_path_safe;

/// Lowercase text of only `a-z`, `0-9`, `.`, `_` and `-`, non-empty (REQ-SAG-0004).
///
/// An invalid value is refused with [`LogError::InvalidSlug`], never rewritten. A slug is one
/// directory name of the session log layout (REQ-SAG-0005), so the path-safe rule also applies:
/// at most [`MAX_SEGMENT_BYTES`](crate::MAX_SEGMENT_BYTES) bytes, not `.` or `..`, no trailing
/// `.`, and not a Windows device name (`con`, `nul`, `com1`, ...).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Slug(Arc<str>);

impl Slug {
    /// Validates `text` as the slug for `field` (`"project"` or `"agent_role"`).
    ///
    /// # Errors
    ///
    /// [`LogError::InvalidSlug`] with `field` and the text when the text has a character outside
    /// `a-z 0-9 . _ -` or is not a path-safe name (empty, too long, `.`, `..`, trailing `.`,
    /// a Windows device name).
    pub fn new(field: &'static str, text: &str) -> Result<Self, LogError> {
        let allowed = text
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-'));
        if !allowed || !is_path_safe(text) {
            return Err(LogError::InvalidSlug {
                field,
                value: text.to_owned(),
            });
        }
        Ok(Self(Arc::from(text)))
    }

    /// The slug text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Slug {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl AsRef<str> for Slug {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
