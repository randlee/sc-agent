//! The grammar table: every tree-sitter grammar this build can load.
//!
//! Adding a grammar is one [`GRAMMARS`] row, one [`KNOWN_LANGUAGES`] entry, its queries and a
//! test sample; the public API does not change.

use crate::Error;
use crate::query::QueryInfo;

/// A compiled-in grammar: its id, loader, and predefined queries.
pub(crate) struct Grammar {
    pub(crate) id: &'static str,
    pub(crate) load: fn() -> tree_sitter::Language,
    pub(crate) queries: &'static [QueryInfo],
}

/// Every language id this crate version can be built with, compiled in or not.
pub(crate) const KNOWN_LANGUAGES: &[&str] = &["python", "rust"];

#[cfg(feature = "grammar-python")]
fn load_python() -> tree_sitter::Language {
    tree_sitter_python::LANGUAGE.into()
}

#[cfg(feature = "grammar-rust")]
fn load_rust() -> tree_sitter::Language {
    tree_sitter_rust::LANGUAGE.into()
}

/// Every grammar compiled into this build, in listing order.
static GRAMMARS: &[Grammar] = &[
    #[cfg(feature = "grammar-python")]
    Grammar {
        id: "python",
        load: load_python,
        queries: crate::query::PYTHON,
    },
    #[cfg(feature = "grammar-rust")]
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
/// # #[cfg(feature = "grammar-python")] {
/// let python = sc_treesitter::LanguageId::new("python")?;
/// assert_eq!(python.as_str(), "python");
/// # }
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
    /// [`Error::GrammarUnavailable`] for a known id whose grammar feature is off, otherwise
    /// [`Error::UnknownLanguage`].
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

impl std::hash::Hash for LanguageId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

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

impl std::str::FromStr for LanguageId {
    type Err = Error;

    fn from_str(id: &str) -> Result<Self, Error> {
        Self::new(id)
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
/// [`Error::GrammarUnavailable`] for a known id whose feature is off, otherwise
/// [`Error::UnknownLanguage`].
fn lookup(id: &str) -> Result<&'static Grammar, Error> {
    GRAMMARS
        .iter()
        .find(|grammar| grammar.id == id)
        .ok_or_else(|| {
            if KNOWN_LANGUAGES.contains(&id) {
                Error::GrammarUnavailable {
                    language: id.to_owned(),
                }
            } else {
                Error::UnknownLanguage {
                    language: id.to_owned(),
                }
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_loaded_language_is_known() {
        for id in languages() {
            assert!(KNOWN_LANGUAGES.contains(&id.as_str()), "{id}");
        }
    }

    #[test]
    fn unknown_id_is_unknown_language() {
        assert!(matches!(
            LanguageId::new("cobol"),
            Err(Error::UnknownLanguage { language }) if language == "cobol"
        ));
    }

    /// AC: REQ-SAG-0302 every id (built with `--no-default-features --features grammar-python`).
    #[cfg(all(feature = "grammar-python", not(feature = "grammar-rust")))]
    #[test]
    fn known_language_without_its_feature_is_grammar_unavailable() {
        let ids: Vec<&str> = languages().map(|id| id.as_str()).collect();
        assert_eq!(ids, ["python"]);
        let error = LanguageId::new("rust").expect_err("rust is not compiled in");
        assert_eq!(
            error,
            Error::GrammarUnavailable {
                language: "rust".to_owned()
            }
        );
        assert_eq!(error.id(), "treesitter.grammar_unavailable");
    }
}
