//! Predefined query assets, one or more per grammar.

use crate::{Error, LanguageId};

/// One predefined query.
///
/// Serializes as `{"language": ..., "query": ...}`; the query text is not serialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub struct QueryInfo {
    /// Grammar id the query is written for.
    pub language: &'static str,
    /// Asset name without the `.scm` suffix.
    #[serde(rename = "query")]
    pub name: &'static str,
    /// Asset text, including the trailing newline.
    #[serde(skip)]
    pub source: &'static str,
}

pub(crate) const PYTHON: &[QueryInfo] = &[QueryInfo {
    language: "python",
    name: "branches",
    source: include_str!("../assets/queries/python/branches.scm"),
}];

pub(crate) const RUST: &[QueryInfo] = &[QueryInfo {
    language: "rust",
    name: "match-arms",
    source: include_str!("../assets/queries/rust/match-arms.scm"),
}];

/// Returns the predefined queries for `language`, or for every compiled-in language.
#[must_use]
pub fn queries(language: Option<LanguageId>) -> Vec<&'static QueryInfo> {
    match language {
        Some(id) => id.grammar().queries.iter().collect(),
        None => crate::languages()
            .flat_map(|id| id.grammar().queries)
            .collect(),
    }
}

/// Returns the text of the predefined query `name` for `language`.
///
/// The returned text is the asset file, including its trailing newline.
///
/// # Errors
///
/// [`Error::UnknownQuery`] when the language has no query by that name.
pub fn query_source(language: LanguageId, name: &str) -> Result<&'static str, Error> {
    language
        .grammar()
        .queries
        .iter()
        .find(|asset| asset.name == name)
        .map(|asset| asset.source)
        .ok_or_else(|| Error::UnknownQuery {
            language,
            name: name.to_owned(),
        })
}
