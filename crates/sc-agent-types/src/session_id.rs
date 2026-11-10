//! [`SessionId`], the ULID that correlates one session's records.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ulid::Ulid;

use crate::TypesError;

/// Session correlation id: a ULID (26 Crockford base32 characters).
///
/// `Display`, `FromStr` and serde all use the 26-character text form. The id
/// carries a millisecond clock, so session file names sort by creation time.
///
/// # Examples
///
/// ```
/// use sc_agent_types::SessionId;
///
/// let id = SessionId::new();
/// assert_eq!(id.to_string().len(), 26);
/// assert_eq!(id.to_string().parse::<SessionId>().unwrap(), id);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(Ulid);

impl SessionId {
    /// Creates a new session id (`Ulid::new()`; no shared generator state).
    ///
    /// `SessionId` does not implement [`Default`]: a default value that is a
    /// fresh random id would surprise callers that expect `default()` to be
    /// deterministic.
    #[must_use]
    #[expect(
        clippy::new_without_default,
        reason = "each call is a new id; a random Default would surprise callers"
    )]
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl FromStr for SessionId {
    type Err = TypesError;

    /// Parses the canonical 26-character uppercase ULID form.
    ///
    /// # Errors
    ///
    /// [`TypesError::InvalidSessionId`] for anything else, a UUID or a lowercase ULID included.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::{ErrorId, SessionId};
    ///
    /// let id: SessionId = "01ARZ3NDEKTSV4RRFFQ69G5FAV".parse().unwrap();
    /// assert_eq!(id.to_string(), "01ARZ3NDEKTSV4RRFFQ69G5FAV");
    /// let uuid = "00000000-0000-0000-0000-000000000000";
    /// assert_eq!(uuid.parse::<SessionId>().unwrap_err().id(), "types.invalid_session_id");
    /// ```
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        parse_ulid(text)
            .map(Self)
            .ok_or_else(|| TypesError::InvalidSessionId {
                text: text.to_owned(),
            })
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

impl Serialize for SessionId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for SessionId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// Strict ULID text: the canonical 26-character uppercase Crockford form.
///
/// `Ulid::from_string` also accepts lowercase, the `I`/`L`/`O` aliases and
/// values above 2^128 (which wrap); comparing with the re-rendered text
/// rejects all three, so one id has one spelling (and one file name).
fn parse_ulid(text: &str) -> Option<Ulid> {
    let ulid = Ulid::from_string(text).ok()?;
    (ulid.to_string() == text).then_some(ulid)
}
