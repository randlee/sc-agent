//! Predefined query assets, one or more per grammar.

use crate::Error;
use crate::grammar;

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

#[cfg(feature = "grammar-python")]
pub(crate) const PYTHON: &[QueryInfo] = &[QueryInfo {
    language: "python",
    name: "branches",
    source: include_str!("../assets/queries/python/branches.scm"),
}];

#[cfg(feature = "grammar-rust")]
pub(crate) const RUST: &[QueryInfo] = &[QueryInfo {
    language: "rust",
    name: "match-arms",
    source: include_str!("../assets/queries/rust/match-arms.scm"),
}];

/// Returns the predefined queries for `language`, or for every compiled-in language.
///
/// # Errors
///
/// [`Error::UnknownLanguage`] or [`Error::GrammarUnavailable`] when `language` is set and
/// that grammar is not compiled into this build.
pub fn queries(language: Option<&str>) -> Result<Vec<&'static QueryInfo>, Error> {
    match language {
        Some(id) => Ok(grammar::lookup(id)?.queries.iter().collect()),
        None => Ok(grammar::all()
            .iter()
            .flat_map(|grammar| grammar.queries)
            .collect()),
    }
}

/// Returns the text of the predefined query `name` for `language`.
///
/// The returned text is the asset file, including its trailing newline.
///
/// # Errors
///
/// [`Error::UnknownLanguage`] or [`Error::GrammarUnavailable`] when the grammar is not
/// compiled in, and [`Error::UnknownQuery`] when the language has no query by that name.
pub fn query_source(language: &str, name: &str) -> Result<&'static str, Error> {
    grammar::lookup(language)?
        .queries
        .iter()
        .find(|asset| asset.name == name)
        .map(|asset| asset.source)
        .ok_or_else(|| Error::UnknownQuery {
            language: language.to_owned(),
            name: name.to_owned(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AC: REQ-SAG-0303 listings.
    #[cfg(all(feature = "grammar-python", feature = "grammar-rust"))]
    #[test]
    fn listings_name_every_predefined_query() {
        let all: Vec<(&str, &str)> = queries(None)
            .expect("all")
            .iter()
            .map(|info| (info.language, info.name))
            .collect();
        assert_eq!(all, [("python", "branches"), ("rust", "match-arms")]);
        assert_eq!(crate::languages().collect::<Vec<_>>(), ["python", "rust"]);
        let json = serde_json::to_value(queries(Some("rust")).expect("rust")).expect("json");
        assert_eq!(
            json,
            serde_json::json!([{"language": "rust", "query": "match-arms"}])
        );
    }

    /// AC: REQ-SAG-0303 listings.
    #[test]
    fn unknown_language_and_query_are_typed() {
        assert_eq!(
            queries(Some("cobol")),
            Err(Error::UnknownLanguage {
                language: "cobol".to_owned()
            })
        );
        #[cfg(feature = "grammar-python")]
        assert_eq!(
            query_source("python", "nope"),
            Err(Error::UnknownQuery {
                language: "python".to_owned(),
                name: "nope".to_owned()
            })
        );
    }
}
