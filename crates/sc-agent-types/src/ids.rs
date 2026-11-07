//! Domain newtypes and [`BackendId`].
//!
//! The backend catalog ([`BACKENDS`], one row of facts per id, REQ-SAG-0105)
//! lives in the private `catalog` module. [`BackendId::descriptor`] holds the
//! only exhaustive `match` over [`BackendId`]; every other read goes through
//! the descriptor.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use crate::catalog::{
    ANTHROPIC_HAIKU, BACKENDS, BackendDescriptor, BackendFamily, JEV, OPENAI_LUNA,
};
use crate::error::JevError;

/// Git toplevel used for path resolution.
///
/// Callers obtain this from `git rev-parse --show-toplevel` (or
/// `git::discover_toplevel` with the `git` feature).
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
///
/// `Deserialize` goes through [`Rev::parse`], so a document cannot carry a
/// revision git would read as an option.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Rev(String);

impl TryFrom<String> for Rev {
    type Error = JevError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse(&text)
    }
}

impl From<Rev> for String {
    fn from(rev: Rev) -> Self {
        rev.0
    }
}

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
            Err(JevError::input_invalid(
                "revision must be WORKTREE or a git revision that is non-empty, does not start with '-', and has no whitespace or NUL",
            ))
        }
    }

    /// Returns the working-tree sentinel.
    #[must_use]
    pub fn worktree() -> Self {
        Self::new(Self::WORKTREE)
    }

    /// Whether this is the [`Self::WORKTREE`] sentinel rather than a git
    /// revision.
    ///
    /// ```
    /// use sc_agent_types::Rev;
    ///
    /// assert!(Rev::worktree().is_worktree());
    /// assert!(!Rev::parse("HEAD").unwrap().is_worktree());
    /// ```
    #[must_use]
    pub fn is_worktree(&self) -> bool {
        self.0 == Self::WORKTREE
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
    #[expect(
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

impl FromStr for SessionId {
    type Err = JevError;

    /// Parses a hyphenated UUID.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::SessionId;
    ///
    /// let id: SessionId = "00000000-0000-0000-0000-000000000000".parse().unwrap();
    /// assert_eq!(id.to_string(), "00000000-0000-0000-0000-000000000000");
    /// assert_eq!("nope".parse::<SessionId>().unwrap_err().code(), "INPUT_INVALID");
    /// ```
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(text)
            .map(Self)
            .map_err(|_| JevError::input_invalid(format!("session id {text:?} is not a UUID")))
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

/// Backend credential id consumed by `sc-agent-secrets`.
///
/// The documented spelling is the [`Display`](fmt::Display) form
/// (`typesafe`, `anthropic`, `openai`). Serde writes that form and reads it
/// through [`FromStr`], which also accepts the one-release alias `open_ai`
/// (bead `agt-backend-serde-alias-removal`).
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

/// Read aliases for each credential id, beside its documented name.
const KEY_ALIASES: &[(BackendKeyId, &[&str])] = &[
    (BackendKeyId::TypeSafe, &[]),
    (BackendKeyId::Anthropic, &[]),
    (BackendKeyId::OpenAi, &["open_ai"]),
];

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
}

impl fmt::Display for BackendKeyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for BackendKeyId {
    type Err = JevError;

    /// Parses the documented id or its read alias (`open_ai`).
    ///
    /// Kebab-case and other spellings are rejected.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        KEY_ALIASES
            .iter()
            .find(|(id, aliases)| id.as_str() == text || aliases.contains(&text))
            .map(|(id, _)| *id)
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

/// Judge implementation id.
///
/// The documented spelling is [`BackendDescriptor::name`]. Serde writes it;
/// `Deserialize` goes through [`FromStr`], which also accepts the
/// descriptor's [`BackendDescriptor::aliases`] (one alias source; there is no
/// `#[serde(alias)]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum BackendId {
    /// TypeSafe Jev.
    Jev,
    /// Anthropic Haiku, API-direct.
    AnthropicHaiku,
    /// OpenAI Luna, API-direct.
    OpenAiLuna,
}

impl BackendId {
    /// Returns this backend's row of [`BACKENDS`].
    ///
    /// This is the one exhaustive `match` over `BackendId` in the workspace.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::{BackendFamily, BackendId, BackendKeyId};
    ///
    /// let luna = BackendId::OpenAiLuna.descriptor();
    /// assert_eq!(luna.name(), "openai_luna");
    /// assert_eq!(luna.key_id(), Some(BackendKeyId::OpenAi));
    /// assert_eq!(luna.family(), BackendFamily::Llm);
    /// ```
    #[must_use]
    pub const fn descriptor(self) -> &'static BackendDescriptor {
        match self {
            Self::Jev => &JEV,
            Self::AnthropicHaiku => &ANTHROPIC_HAIKU,
            Self::OpenAiLuna => &OPENAI_LUNA,
        }
    }

    /// The documented spelling ([`BackendDescriptor::name`]).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.descriptor().name()
    }

    /// The credential this backend sends ([`BackendDescriptor::key_id`]).
    #[must_use]
    pub const fn key_id(self) -> Option<BackendKeyId> {
        self.descriptor().key_id()
    }

    /// The backend family ([`BackendDescriptor::family`]).
    #[must_use]
    pub const fn family(self) -> BackendFamily {
        self.descriptor().family()
    }
}

impl FromStr for BackendId {
    type Err = JevError;

    /// Parses a descriptor name or alias.
    ///
    /// `jev`, `anthropic_haiku`, and `openai_luna` are the documented ids;
    /// `open_ai_luna` is a one-release read alias. Kebab-case spellings such
    /// as `anthropic-haiku` are rejected.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::BackendId;
    ///
    /// assert_eq!("openai_luna".parse::<BackendId>().unwrap(), BackendId::OpenAiLuna);
    /// assert_eq!("open_ai_luna".parse::<BackendId>().unwrap(), BackendId::OpenAiLuna);
    /// assert_eq!("openai-luna".parse::<BackendId>().unwrap_err().code(), "INPUT_INVALID");
    /// ```
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        BACKENDS
            .iter()
            .find(|row| row.name() == text || row.aliases().contains(&text))
            .map(BackendDescriptor::id)
            .ok_or_else(|| {
                JevError::input_invalid(format!(
                    "backend {text} is not jev, anthropic_haiku, or openai_luna"
                ))
            })
    }
}

impl fmt::Display for BackendId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for BackendId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for BackendId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// A non-empty, duplicate-free list of backends, in request order.
///
/// # Examples
///
/// ```
/// use sc_agent_types::{BackendId, BackendSet};
///
/// let set: BackendSet = "jev,anthropic_haiku".parse().unwrap();
/// assert_eq!(set.as_slice(), [BackendId::Jev, BackendId::AnthropicHaiku]);
/// assert_eq!("jev,jev".parse::<BackendSet>().unwrap_err().code(), "CONFIG_INVALID");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendSet(Vec<BackendId>);

impl BackendSet {
    /// The backends, in request order.
    #[must_use]
    pub fn as_slice(&self) -> &[BackendId] {
        &self.0
    }

    /// Iterates the backends in request order.
    pub fn iter(&self) -> impl Iterator<Item = BackendId> + '_ {
        self.0.iter().copied()
    }

    /// Number of backends (never zero).
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Always false: a `BackendSet` has at least one backend.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl TryFrom<Vec<BackendId>> for BackendSet {
    type Error = JevError;

    /// # Errors
    ///
    /// `CONFIG_INVALID` when the list is empty or repeats a backend.
    fn try_from(ids: Vec<BackendId>) -> Result<Self, Self::Error> {
        if ids.is_empty() {
            return Err(JevError::config_invalid(
                "the backend list is empty; name at least one of jev, anthropic_haiku, or openai_luna",
            ));
        }
        let mut seen = BTreeSet::new();
        for id in &ids {
            if !seen.insert(*id) {
                return Err(JevError::config_invalid(format!(
                    "backend {id} is listed more than once"
                )));
            }
        }
        Ok(Self(ids))
    }
}

impl FromStr for BackendSet {
    type Err = JevError;

    /// Parses a comma-separated list (`jev,anthropic_haiku`).
    ///
    /// Surrounding whitespace on each entry is ignored.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let ids = text
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::parse)
            .collect::<Result<Vec<BackendId>, JevError>>()?;
        Self::try_from(ids)
    }
}

impl AsRef<[BackendId]> for BackendSet {
    fn as_ref(&self) -> &[BackendId] {
        self.as_slice()
    }
}

/// Lowercase hex SHA-256 (64 characters).
///
/// Used for canonical payload digests and equal-input `prompt_sha256`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
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
        if text.len() == 64 && is_lower_hex(text) {
            Ok(Self(text.to_owned()))
        } else {
            Err(JevError::input_invalid(
                "SHA-256 digest must be 64 lowercase hex characters",
            ))
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

/// Git object id of a blob: 40 (SHA-1) or 64 (SHA-256) lowercase hex digits.
///
/// # Examples
///
/// ```
/// use sc_agent_types::BlobSha;
///
/// assert!("a".repeat(40).parse::<BlobSha>().is_ok());
/// assert!("A".repeat(40).parse::<BlobSha>().is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BlobSha(String);

impl BlobSha {
    /// Returns the lowercase hex text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for BlobSha {
    type Error = JevError;

    /// # Errors
    ///
    /// `INPUT_INVALID` unless the text is 40 or 64 lowercase hex digits.
    fn try_from(text: &str) -> Result<Self, Self::Error> {
        if matches!(text.len(), 40 | 64) && is_lower_hex(text) {
            Ok(Self(text.to_owned()))
        } else {
            Err(JevError::input_invalid(format!(
                "blob sha {text:?} is not 40 or 64 lowercase hex digits"
            )))
        }
    }
}

impl TryFrom<String> for BlobSha {
    type Error = JevError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::try_from(text.as_str())
    }
}

impl FromStr for BlobSha {
    type Err = JevError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::try_from(text)
    }
}

impl From<BlobSha> for String {
    fn from(value: BlobSha) -> Self {
        value.0
    }
}

impl fmt::Display for BlobSha {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A path relative to the repository root, with `/` separators.
///
/// Validated repo-relative path: non-empty, not absolute, at least one
/// normal component, no `..`, and no `\`, NUL, or `:`. The text is kept as
/// written. In r-1 it guards `sc-jev-check`'s file reads, subjects documents
/// and the `git` helpers; the `check.toml` path fields (r-2) and
/// `sc-treesitter`'s CLI path handling (r-3c) adopt it later.
///
/// # Examples
///
/// ```
/// use sc_agent_types::RepoRelativePath;
///
/// assert_eq!(RepoRelativePath::try_from("src/a.py").unwrap().as_str(), "src/a.py");
/// for bad in ["", "/etc/passwd", "../x", "a\\b", "c:x"] {
///     assert_eq!(RepoRelativePath::try_from(bad).unwrap_err().code(), "INPUT_INVALID");
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RepoRelativePath(String);

impl RepoRelativePath {
    /// Returns the path text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the path as a [`Path`].
    #[must_use]
    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }
}

impl TryFrom<&str> for RepoRelativePath {
    type Error = JevError;

    /// # Errors
    ///
    /// `INPUT_INVALID` for empty text, an absolute path, a `..` component, or
    /// a `\`, NUL, or `:` character.
    fn try_from(text: &str) -> Result<Self, Self::Error> {
        let path = Path::new(text);
        let well_formed = !text.is_empty()
            && !text.starts_with('/')
            && !text.contains(['\\', '\0', ':'])
            && !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
            && path
                .components()
                .any(|component| matches!(component, Component::Normal(_)));
        if well_formed {
            Ok(Self(text.to_owned()))
        } else {
            Err(JevError::input_invalid(format!(
                "path {text} must be a repo-relative path inside the repository (no leading /, .., \\, NUL, or :)"
            )))
        }
    }
}

impl TryFrom<String> for RepoRelativePath {
    type Error = JevError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::try_from(text.as_str())
    }
}

impl FromStr for RepoRelativePath {
    type Err = JevError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::try_from(text)
    }
}

impl From<RepoRelativePath> for String {
    fn from(value: RepoRelativePath) -> Self {
        value.0
    }
}

impl fmt::Display for RepoRelativePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl PartialEq<str> for RepoRelativePath {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for RepoRelativePath {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl AsRef<str> for RepoRelativePath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

macro_rules! label_newtype {
    ($(#[$meta:meta])* $name:ident, $what:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            /// Returns the text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<&str> for $name {
            type Error = JevError;

            /// # Errors
            ///
            /// `INPUT_INVALID` when the text is empty or has a control character.
            fn try_from(text: &str) -> Result<Self, Self::Error> {
                if text.is_empty() || text.chars().any(char::is_control) {
                    Err(JevError::input_invalid(format!(
                        concat!($what, " {:?} must be non-empty and have no control characters"),
                        text
                    )))
                } else {
                    Ok(Self(text.to_owned()))
                }
            }
        }

        impl TryFrom<String> for $name {
            type Error = JevError;

            fn try_from(text: String) -> Result<Self, Self::Error> {
                if text.is_empty() || text.chars().any(char::is_control) {
                    Self::try_from(text.as_str())
                } else {
                    Ok(Self(text))
                }
            }
        }

        impl FromStr for $name {
            type Err = JevError;

            fn from_str(text: &str) -> Result<Self, Self::Err> {
                Self::try_from(text)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl PartialEq<str> for $name {
            fn eq(&self, other: &str) -> bool {
                self.as_str() == other
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.as_str() == *other
            }
        }

        impl std::borrow::Borrow<str> for $name {
            fn borrow(&self) -> &str {
                self.as_str()
            }
        }
    };
}

label_newtype!(
    /// Caller-chosen question id (non-empty, no control characters).
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::QuestionId;
    ///
    /// assert_eq!(QuestionId::try_from("relevant").unwrap().as_str(), "relevant");
    /// assert!(QuestionId::try_from("").is_err());
    /// ```
    QuestionId,
    "question id"
);

label_newtype!(
    /// One choice label of a question (non-empty, no control characters).
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::ChoiceId;
    ///
    /// assert_eq!(ChoiceId::try_from("yes").unwrap().as_str(), "yes");
    /// assert!(ChoiceId::try_from("a\nb").is_err());
    /// ```
    ChoiceId,
    "choice"
);

/// The send a log event records: a judge send or a session `ask`.
///
/// Log adapters write [`Self::as_str`] (`judge` or `ask`), which is also a
/// valid `sc-observability` action name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Operation {
    /// A judge send of a check run.
    Judge,
    /// A session `ask` send.
    Ask,
}

impl Operation {
    /// Every operation, in declaration order.
    pub const ALL: &'static [Self] = &[Self::Judge, Self::Ask];

    /// The wire and action-name spelling: `judge` or `ask`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Judge => "judge",
            Self::Ask => "ask",
        }
    }
}

impl fmt::Display for Operation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Operation {
    type Err = JevError;

    /// Parses the [`Self::as_str`] spelling.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_types::Operation;
    ///
    /// assert_eq!("ask".parse::<Operation>().unwrap(), Operation::Ask);
    /// assert_eq!("Judge".parse::<Operation>().unwrap_err().code(), "INPUT_INVALID");
    /// ```
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .copied()
            .find(|operation| operation.as_str() == text)
            .ok_or_else(|| {
                JevError::input_invalid(format!("log operation must be judge or ask, not {text:?}"))
            })
    }
}

fn is_lower_hex(text: &str) -> bool {
    text.bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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
    fn descriptor_lookup_matches_catalog_rows() {
        for row in BACKENDS {
            assert_eq!(row.id().descriptor(), row);
        }
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
