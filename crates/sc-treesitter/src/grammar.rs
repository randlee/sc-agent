//! Pinned, feature-flagged grammars.
//!
//! Phase A loads grammars at compile time. There is no dynamic grammar
//! loading. `python` and `rust` are default features. Further grammars belong
//! behind their own Cargo features and stay out of the default set.

use sc_agent_types::JevError;

/// One grammar compiled into this build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageInfo {
    /// CLI language id (`--language`).
    pub id: &'static str,
    /// Grammar crate name recorded in `producer.grammar`.
    pub grammar: &'static str,
    /// Pinned grammar crate version recorded in `producer.grammar_version`.
    pub grammar_version: &'static str,
}

#[cfg(feature = "python")]
const PYTHON: LanguageInfo = LanguageInfo {
    id: "python",
    grammar: "tree-sitter-python",
    grammar_version: "0.25.0",
};

#[cfg(feature = "rust")]
const RUST: LanguageInfo = LanguageInfo {
    id: "rust",
    grammar: "tree-sitter-rust",
    grammar_version: "0.24.2",
};

#[cfg(all(feature = "python", feature = "rust"))]
const LANGUAGES: &[LanguageInfo] = &[PYTHON, RUST];

#[cfg(all(feature = "python", not(feature = "rust")))]
const LANGUAGES: &[LanguageInfo] = &[PYTHON];

#[cfg(all(not(feature = "python"), feature = "rust"))]
const LANGUAGES: &[LanguageInfo] = &[RUST];

#[cfg(not(any(feature = "python", feature = "rust")))]
const LANGUAGES: &[LanguageInfo] = &[];

/// Returns the grammars compiled into this build, in CLI order.
#[must_use]
pub fn languages() -> &'static [LanguageInfo] {
    LANGUAGES
}

/// Returns the pinned grammar named `id`.
///
/// # Errors
///
/// Returns [`JevError::LanguageUnavailable`] when `id` is not a compiled-in
/// grammar. Disabling the Cargo feature for a grammar makes that id unavailable.
pub fn language(id: &str) -> Result<&'static LanguageInfo, JevError> {
    languages()
        .iter()
        .find(|info| info.id == id)
        .ok_or_else(|| JevError::LanguageUnavailable {
            language: id.to_owned(),
        })
}

pub(crate) fn load(id: &str) -> Result<tree_sitter::Language, JevError> {
    let info = language(id)?;
    match info.id {
        #[cfg(feature = "python")]
        "python" => Ok(tree_sitter_python::LANGUAGE.into()),
        #[cfg(feature = "rust")]
        "rust" => Ok(tree_sitter_rust::LANGUAGE.into()),
        other => Err(JevError::LanguageUnavailable {
            language: other.to_owned(),
        }),
    }
}
