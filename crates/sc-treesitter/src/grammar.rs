//! Pinned, feature-flagged grammars.
//!
//! Phase A loads grammars at compile time. There is no dynamic grammar
//! loading. `python` and `rust` are default features. Further grammars belong
//! behind their own Cargo features and stay out of the default set.
//!
//! [`GRAMMARS`] is the one table: each element carries its metadata, its
//! loader, and its query assets, and is `#[cfg]`-gated on its own feature.

use sc_agent_types::JevError;

use crate::query::QueryInfo;

/// One grammar compiled into this build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct LanguageInfo {
    /// CLI language id (`--language`).
    pub id: &'static str,
    /// Grammar crate name recorded in `producer.grammar`.
    pub grammar: &'static str,
    /// Pinned grammar crate version recorded in `producer.grammar_version`.
    pub grammar_version: &'static str,
}

/// A compiled-in grammar: metadata, loader, and its query assets.
pub(crate) struct Grammar {
    pub(crate) info: LanguageInfo,
    pub(crate) load: fn() -> tree_sitter::Language,
    pub(crate) queries: &'static [QueryInfo],
}

#[cfg(feature = "python")]
fn load_python() -> tree_sitter::Language {
    tree_sitter_python::LANGUAGE.into()
}

#[cfg(feature = "rust")]
fn load_rust() -> tree_sitter::Language {
    tree_sitter_rust::LANGUAGE.into()
}

/// Every grammar compiled into this build, in CLI order.
static GRAMMARS: &[Grammar] = &[
    #[cfg(feature = "python")]
    Grammar {
        info: LanguageInfo {
            id: "python",
            grammar: "tree-sitter-python",
            grammar_version: "0.25.0",
        },
        load: load_python,
        queries: crate::query::PYTHON,
    },
    #[cfg(feature = "rust")]
    Grammar {
        info: LanguageInfo {
            id: "rust",
            grammar: "tree-sitter-rust",
            grammar_version: "0.24.2",
        },
        load: load_rust,
        queries: crate::query::RUST,
    },
];

/// Returns the grammars compiled into this build, in CLI order.
///
/// # Examples
///
/// ```
/// let ids: Vec<&str> = sc_treesitter::languages().map(|info| info.id).collect();
/// assert!(ids.iter().all(|id| sc_treesitter::language(id).is_ok()));
/// ```
pub fn languages() -> impl ExactSizeIterator<Item = &'static LanguageInfo> {
    GRAMMARS.iter().map(|grammar| &grammar.info)
}

/// Returns the pinned grammar named `id`.
///
/// # Errors
///
/// Returns [`JevError::LanguageUnavailable`] when `id` is not a compiled-in
/// grammar. Disabling the Cargo feature for a grammar makes that id unavailable.
pub fn language(id: &str) -> Result<&'static LanguageInfo, JevError> {
    lookup(id).map(|grammar| &grammar.info)
}

/// Single lookup into [`GRAMMARS`] by CLI id.
pub(crate) fn lookup(id: &str) -> Result<&'static Grammar, JevError> {
    GRAMMARS
        .iter()
        .find(|grammar| grammar.info.id == id)
        .ok_or_else(|| JevError::language_unavailable(crate::PRODUCER_NAME, id))
}

/// Every compiled-in grammar, for listing queries.
pub(crate) fn all() -> &'static [Grammar] {
    GRAMMARS
}
