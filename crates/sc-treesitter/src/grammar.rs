//! The grammar table: every tree-sitter grammar this build can load.
//!
//! Adding a grammar is one [`GRAMMARS`] row, its queries and a test sample; the public API does
//! not change.

use crate::Error;
use crate::query::QueryInfo;

/// A compiled-in grammar: its id, loader, and predefined queries.
pub(crate) struct Grammar {
    pub(crate) id: &'static str,
    pub(crate) load: fn() -> tree_sitter::Language,
    pub(crate) queries: &'static [QueryInfo],
}

fn load_python() -> tree_sitter::Language {
    tree_sitter_python::LANGUAGE.into()
}

fn load_rust() -> tree_sitter::Language {
    tree_sitter_rust::LANGUAGE.into()
}

/// Every grammar compiled into this build, in listing order.
static GRAMMARS: &[Grammar] = &[
    Grammar {
        id: "python",
        load: load_python,
        queries: crate::query::PYTHON,
    },
    Grammar {
        id: "rust",
        load: load_rust,
        queries: crate::query::RUST,
    },
];

/// A language id checked once against the grammars compiled into this build.
///
/// The only way to get one is [`LanguageId::new`] (or [`languages`]), so every function that
/// takes a `LanguageId` knows its grammar is loaded. It serializes and displays as the id.
///
/// # Examples
///
/// ```
/// let python = sc_treesitter::LanguageId::new("python")?;
/// assert_eq!(python.as_str(), "python");
/// assert!(sc_treesitter::LanguageId::new("cobol").is_err());
/// # Ok::<(), sc_treesitter::Error>(())
/// ```
#[derive(Clone, Copy)]
pub struct LanguageId {
    grammar: &'static Grammar,
}

impl LanguageId {
    /// Checks `id` against the compiled-in grammars.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownLanguage`] when no loaded grammar has that id.
    pub fn new(id: &str) -> Result<Self, Error> {
        lookup(id).map(|grammar| Self { grammar })
    }

    /// The id, for example `"python"`.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        self.grammar.id
    }

    /// The grammar this id names.
    pub(crate) fn grammar(&self) -> &'static Grammar {
        self.grammar
    }
}

impl PartialEq for LanguageId {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for LanguageId {}

impl std::fmt::Debug for LanguageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("LanguageId").field(&self.as_str()).finish()
    }
}

impl std::fmt::Display for LanguageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl serde::Serialize for LanguageId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// Returns the grammars compiled into this build, in listing order.
///
/// # Examples
///
/// ```
/// for language in sc_treesitter::languages() {
///     assert_eq!(sc_treesitter::LanguageId::new(language.as_str())?, language);
/// }
/// # Ok::<(), sc_treesitter::Error>(())
/// ```
pub fn languages() -> impl ExactSizeIterator<Item = LanguageId> {
    GRAMMARS.iter().map(|grammar| LanguageId { grammar })
}

/// Single lookup into [`GRAMMARS`] by language id.
///
/// [`Error::UnknownLanguage`] when no loaded grammar has that id.
fn lookup(id: &str) -> Result<&'static Grammar, Error> {
    GRAMMARS
        .iter()
        .find(|grammar| grammar.id == id)
        .ok_or_else(|| Error::UnknownLanguage {
            language: id.to_owned(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_id_is_unknown_language() {
        assert!(matches!(
            LanguageId::new("cobol"),
            Err(Error::UnknownLanguage { language }) if language == "cobol"
        ));
    }
}
