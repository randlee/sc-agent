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

/// Returns the ids of the grammars compiled into this build, in listing order.
///
/// # Examples
///
/// ```
/// let ids: Vec<&str> = sc_treesitter::languages().collect();
/// for id in &ids {
///     assert!(sc_treesitter::queries(Some(id)).is_ok());
/// }
/// ```
pub fn languages() -> impl ExactSizeIterator<Item = &'static str> {
    GRAMMARS.iter().map(|grammar| grammar.id)
}

/// Single lookup into [`GRAMMARS`] by language id.
///
/// [`Error::GrammarUnavailable`] for a known id whose feature is off, otherwise
/// [`Error::UnknownLanguage`].
pub(crate) fn lookup(id: &str) -> Result<&'static Grammar, Error> {
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

/// Every compiled-in grammar, for listing queries.
pub(crate) fn all() -> &'static [Grammar] {
    GRAMMARS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_loaded_language_is_known() {
        for id in languages() {
            assert!(KNOWN_LANGUAGES.contains(&id), "{id}");
        }
    }

    #[test]
    fn unknown_id_is_unknown_language() {
        assert!(matches!(
            lookup("cobol"),
            Err(Error::UnknownLanguage { language }) if language == "cobol"
        ));
    }

    /// AC: REQ-SAG-0302 every id (built with `--no-default-features --features grammar-python`).
    #[cfg(all(feature = "grammar-python", not(feature = "grammar-rust")))]
    #[test]
    fn known_language_without_its_feature_is_grammar_unavailable() {
        assert_eq!(languages().collect::<Vec<_>>(), ["python"]);
        let error = lookup("rust").err().expect("rust is not compiled in");
        assert_eq!(
            error,
            Error::GrammarUnavailable {
                language: "rust".to_owned()
            }
        );
        assert_eq!(error.id(), "treesitter.grammar_unavailable");
    }
}
