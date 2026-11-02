//! Domain newtypes frozen in sprint a-1.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::JevError;

/// Git toplevel used for path resolution.
///
/// Callers obtain this from `git rev-parse --show-toplevel`. This type does
/// not run git.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRoot(PathBuf);

impl RepoRoot {
    /// Wraps a git toplevel path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    /// Returns the toplevel path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for RepoRoot {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

/// Git revision, or the literal `WORKTREE`, for `git show` inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Rev(String);

impl Rev {
    /// Revision string meaning the working tree rather than a commit.
    pub const WORKTREE: &'static str = "WORKTREE";

    /// Wraps a git revision or [`Self::WORKTREE`].
    #[must_use]
    pub fn new(rev: impl Into<String>) -> Self {
        Self(rev.into())
    }

    /// Returns the working-tree sentinel.
    #[must_use]
    pub fn worktree() -> Self {
        Self::new(Self::WORKTREE)
    }

    /// Returns the revision text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Rev {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Session correlation id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(Uuid);

impl SessionId {
    /// Creates a new random session id.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wraps an existing UUID.
    #[must_use]
    pub const fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    /// Returns the inner UUID.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

/// Backend credential id consumed by `sc-agent-secrets`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendKeyId {
    /// TypeSafe Jev API credential.
    TypeSafe,
    /// Anthropic API credential.
    Anthropic,
    /// OpenAI API credential.
    OpenAi,
}

impl fmt::Display for BackendKeyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TypeSafe => "typesafe",
            Self::Anthropic => "anthropic",
            Self::OpenAi => "openai",
        })
    }
}

/// Judge implementation id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendId {
    /// TypeSafe Jev.
    Jev,
    /// Anthropic Haiku, API-direct.
    AnthropicHaiku,
    /// OpenAI Luna, API-direct.
    OpenAiLuna,
}

impl fmt::Display for BackendId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Jev => "jev",
            Self::AnthropicHaiku => "anthropic_haiku",
            Self::OpenAiLuna => "openai_luna",
        })
    }
}

/// Lowercase hex SHA-256 (64 characters).
///
/// Used for canonical payload digests and equal-input `prompt_sha256`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Sha256Digest(String);

impl Sha256Digest {
    /// Builds a digest from 32 raw SHA-256 bytes.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(encode_hex(&bytes))
    }

    /// Hashes `bytes` with SHA-256.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(bytes);
        let mut raw = [0_u8; 32];
        raw.copy_from_slice(&digest);
        Self::from_bytes(raw)
    }

    /// Parses 64 lowercase hex characters.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::InputInvalid`] when the text is not 64 lowercase
    /// hex characters.
    pub fn parse(text: impl AsRef<str>) -> Result<Self, JevError> {
        let text = text.as_ref();
        if text.len() == 64
            && text
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            Ok(Self(text.to_owned()))
        } else {
            Err(JevError::InputInvalid {
                detail: "SHA-256 digest must be 64 lowercase hex characters".into(),
            })
        }
    }

    /// Returns the lowercase hex text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl AsRef<str> for Sha256Digest {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<Sha256Digest> for String {
    fn from(value: Sha256Digest) -> Self {
        value.0
    }
}

impl TryFrom<String> for Sha256Digest {
    type Error = JevError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_rejects_uppercase_and_short_text() {
        assert!(Sha256Digest::parse("ab").is_err());
        assert!(Sha256Digest::parse("A".repeat(64)).is_err());
        let ok = "ab".repeat(32);
        assert_eq!(Sha256Digest::parse(&ok).unwrap().as_str(), ok);
    }

    #[test]
    fn digest_of_empty_is_stable() {
        assert_eq!(
            Sha256Digest::of(b"").as_str(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
