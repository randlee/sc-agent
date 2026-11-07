//! Domain newtypes frozen in sprint a-1.

use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

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

    /// Wraps a git revision or [`Self::WORKTREE`] without validation.
    ///
    /// Use this for revisions the program builds itself, such as
    /// [`Self::worktree`] or a SHA read back from git. Text that comes from a
    /// caller (CLI flags, Python arguments, `check.toml`) goes through
    /// [`Self::parse`], which is the edge entry point.
    #[must_use]
    pub fn new(rev: impl Into<String>) -> Self {
        Self(rev.into())
    }

    /// Parses caller-supplied revision text.
    ///
    /// Accepts [`Self::WORKTREE`] or any text git may resolve. Rejects text
    /// that git would read as an option or that cannot be one argv element:
    /// empty text, a leading `-`, NUL, or any whitespace. This check does not
    /// run git, so a well-formed name can still fail to resolve later.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::InputInvalid`] when the text is empty, starts with
    /// `-`, or contains NUL or whitespace.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::Rev;
    ///
    /// assert_eq!(Rev::parse("HEAD~1").unwrap().as_str(), "HEAD~1");
    /// assert_eq!(Rev::parse("WORKTREE").unwrap(), Rev::worktree());
    /// assert_eq!(Rev::parse("--output=x").unwrap_err().code(), "INPUT_INVALID");
    /// ```
    pub fn parse(text: &str) -> Result<Self, JevError> {
        let well_formed = !text.is_empty()
            && !text.starts_with('-')
            && !text.chars().any(|ch| ch == '\0' || ch.is_whitespace());
        if well_formed {
            Ok(Self(text.to_owned()))
        } else {
            Err(JevError::InputInvalid {
                detail: "revision must be WORKTREE or a git revision that is non-empty, does not start with '-', and has no whitespace or NUL".to_owned(),
            })
        }
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
    ///
    /// `SessionId` does not implement [`Default`]: a default value that is a
    /// fresh random UUID would surprise callers that expect `default()` to be
    /// deterministic.
    #[must_use]
    #[allow(
        clippy::new_without_default,
        reason = "a random id is not a sensible Default value"
    )]
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

impl FromStr for BackendKeyId {
    type Err = JevError;

    /// Parses the documented id: the [`Display`](fmt::Display) form.
    ///
    /// Only `typesafe`, `anthropic`, and `openai` are accepted. Kebab-case or
    /// other aliases are not part of the contract.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "typesafe" => Ok(Self::TypeSafe),
            "anthropic" => Ok(Self::Anthropic),
            "openai" => Ok(Self::OpenAi),
            _ => Err(JevError::InputInvalid {
                detail: format!("backend key {text} is not typesafe, anthropic, or openai"),
            }),
        }
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

impl BackendId {
    /// Returns the credential this backend sends.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::{BackendId, BackendKeyId};
    ///
    /// assert_eq!(BackendId::Jev.key_id(), BackendKeyId::TypeSafe);
    /// assert_eq!(BackendId::OpenAiLuna.key_id(), BackendKeyId::OpenAi);
    /// ```
    #[must_use]
    pub const fn key_id(self) -> BackendKeyId {
        match self {
            Self::Jev => BackendKeyId::TypeSafe,
            Self::AnthropicHaiku => BackendKeyId::Anthropic,
            Self::OpenAiLuna => BackendKeyId::OpenAi,
        }
    }

    /// Returns whether this backend is an LLM judge that needs a system prompt.
    ///
    /// Haiku and Luna read [`crate::JudgeRequest::llm`]; Jev ignores it.
    #[must_use]
    pub const fn is_llm(self) -> bool {
        match self {
            Self::Jev => false,
            Self::AnthropicHaiku | Self::OpenAiLuna => true,
        }
    }
}

impl FromStr for BackendId {
    type Err = JevError;

    /// Parses the documented id: the [`Display`](fmt::Display) form.
    ///
    /// Only `jev`, `anthropic_haiku`, and `openai_luna` are accepted, so the
    /// CLI, `check.toml`, and Python share one spelling. Kebab-case aliases
    /// such as `anthropic-haiku` are deliberately rejected. The serde form of
    /// [`Self::OpenAiLuna`] is currently `open_ai_luna` (a `snake_case`
    /// artifact); that form is not accepted here.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::BackendId;
    ///
    /// assert_eq!("openai_luna".parse::<BackendId>().unwrap(), BackendId::OpenAiLuna);
    /// assert_eq!("openai-luna".parse::<BackendId>().unwrap_err().code(), "INPUT_INVALID");
    /// ```
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "jev" => Ok(Self::Jev),
            "anthropic_haiku" => Ok(Self::AnthropicHaiku),
            "openai_luna" => Ok(Self::OpenAiLuna),
            _ => Err(JevError::InputInvalid {
                detail: format!("backend {text} is not jev, anthropic_haiku, or openai_luna"),
            }),
        }
    }
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
    fn backend_ids_round_trip_through_display_and_from_str() {
        for id in [
            BackendId::Jev,
            BackendId::AnthropicHaiku,
            BackendId::OpenAiLuna,
        ] {
            assert_eq!(id.to_string().parse::<BackendId>().unwrap(), id);
            assert_eq!(id.is_llm(), id != BackendId::Jev);
        }
        for key in [
            BackendKeyId::TypeSafe,
            BackendKeyId::Anthropic,
            BackendKeyId::OpenAi,
        ] {
            assert_eq!(key.to_string().parse::<BackendKeyId>().unwrap(), key);
        }
        assert_eq!(BackendId::AnthropicHaiku.key_id(), BackendKeyId::Anthropic);
    }

    /// Pins a known divergence: `rename_all = "snake_case"` splits `OpenAi`
    /// into `open_ai`, so the serde form differs from `Display`, `FromStr`,
    /// and the docs (`openai_luna`, `openai`). Changing serde changes the
    /// wire form of `JudgeOutcome.backend`, which is a contract decision, so
    /// this test records today's behaviour instead of fixing it.
    #[test]
    fn serde_form_of_openai_ids_differs_from_display() {
        let wire = |value| serde_json::to_value(value).unwrap();
        assert_eq!(wire(BackendId::Jev), "jev");
        assert_eq!(wire(BackendId::AnthropicHaiku), "anthropic_haiku");
        assert_eq!(wire(BackendId::OpenAiLuna), "open_ai_luna");
        assert_eq!(
            serde_json::to_value(BackendKeyId::OpenAi).unwrap(),
            "open_ai"
        );
        assert_eq!(BackendId::OpenAiLuna.to_string(), "openai_luna");
        assert!("open_ai_luna".parse::<BackendId>().is_err());
    }

    #[test]
    fn backend_from_str_rejects_aliases_with_the_cli_message() {
        for text in ["anthropic-haiku", "openai-luna", "Jev", "", "haiku"] {
            let error = text.parse::<BackendId>().unwrap_err();
            assert_eq!(error.code(), "INPUT_INVALID");
            assert_eq!(
                error.to_string(),
                format!(
                    "input is invalid: backend {text} is not jev, anthropic_haiku, or openai_luna"
                )
            );
        }
        let error = "open-ai".parse::<BackendKeyId>().unwrap_err();
        assert_eq!(error.code(), "INPUT_INVALID");
    }

    #[test]
    fn rev_parse_rejects_option_like_and_unsafe_text() {
        for good in [
            "WORKTREE",
            "HEAD",
            "HEAD~1",
            "main",
            "a1b2c3",
            "refs/tags/v1.0",
        ] {
            assert_eq!(Rev::parse(good).unwrap().as_str(), good);
        }
        for bad in [
            "",
            "-",
            "--output=/tmp/x",
            "-n1",
            "HEAD 1",
            "a\tb",
            "a\nb",
            "a\0b",
            " HEAD",
        ] {
            let error = Rev::parse(bad).unwrap_err();
            assert_eq!(error.code(), "INPUT_INVALID", "{bad:?}");
        }
    }

    #[test]
    fn digest_of_empty_is_stable() {
        assert_eq!(
            Sha256Digest::of(b"").as_str(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
