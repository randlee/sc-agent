//! The static backend catalog: [`BACKENDS`] rows of [`BackendDescriptor`].
//!
//! Re-exported through `ids` and the crate root; [`BackendId::descriptor`] is
//! the one lookup.

use crate::ids::{BackendId, BackendKeyId};

/// Broad class of a judge backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BackendFamily {
    /// The TypeSafe Jev API.
    TypeSafeJev,
    /// An API-direct LLM judge that needs a system prompt.
    Llm,
    /// A local judge (no network credential).
    Local,
}

/// Static facts about one built-in backend (one row of [`BACKENDS`]).
///
/// Fields are private; read them with the `const fn` accessors. Adding a
/// backend is a new row, not a new `match` (ADR-SAG-0027 checklist).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendDescriptor {
    id: BackendId,
    name: &'static str,
    aliases: &'static [&'static str],
    key_id: Option<BackendKeyId>,
    family: BackendFamily,
    requires_llm_context: bool,
    supports_session_ask: bool,
    request_limit_bytes: u64,
}

impl BackendDescriptor {
    /// The backend id this row describes.
    #[must_use]
    pub const fn id(&self) -> BackendId {
        self.id
    }

    /// The documented spelling (`Display`, serde, CLI, `check.toml`).
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Spellings accepted on read in addition to [`Self::name`].
    #[must_use]
    pub const fn aliases(&self) -> &'static [&'static str] {
        self.aliases
    }

    /// The credential the backend sends, if any.
    #[must_use]
    pub const fn key_id(&self) -> Option<BackendKeyId> {
        self.key_id
    }

    /// The backend family.
    #[must_use]
    pub const fn family(&self) -> BackendFamily {
        self.family
    }

    /// Whether a request must carry `JudgeRequest::llm` (a system prompt).
    #[must_use]
    pub const fn requires_llm_context(&self) -> bool {
        self.requires_llm_context
    }

    /// Whether a session for this backend accepts a follow-up `ask`.
    #[must_use]
    pub const fn supports_session_ask(&self) -> bool {
        self.supports_session_ask
    }

    /// Largest wire request this backend sends, in bytes.
    #[must_use]
    pub const fn request_limit_bytes(&self) -> u64 {
        self.request_limit_bytes
    }
}

/// The backend catalog: one row per built-in [`BackendId`].
///
/// `test_backend_catalog_roundtrip` checks that serde, `Display`, `FromStr`
/// and the aliases agree for every row.
pub static BACKENDS: &[BackendDescriptor] = &[JEV, ANTHROPIC_HAIKU, OPENAI_LUNA];

pub(crate) const JEV: BackendDescriptor = BackendDescriptor {
    id: BackendId::Jev,
    name: "jev",
    aliases: &[],
    key_id: Some(BackendKeyId::TypeSafe),
    family: BackendFamily::TypeSafeJev,
    requires_llm_context: false,
    supports_session_ask: false,
    request_limit_bytes: 96_000,
};

pub(crate) const ANTHROPIC_HAIKU: BackendDescriptor = BackendDescriptor {
    id: BackendId::AnthropicHaiku,
    name: "anthropic_haiku",
    aliases: &[],
    key_id: Some(BackendKeyId::Anthropic),
    family: BackendFamily::Llm,
    requires_llm_context: true,
    supports_session_ask: true,
    request_limit_bytes: 96_000,
};

pub(crate) const OPENAI_LUNA: BackendDescriptor = BackendDescriptor {
    id: BackendId::OpenAiLuna,
    name: "openai_luna",
    aliases: &["open_ai_luna"],
    key_id: Some(BackendKeyId::OpenAi),
    family: BackendFamily::Llm,
    requires_llm_context: true,
    supports_session_ask: true,
    request_limit_bytes: 96_000,
};
