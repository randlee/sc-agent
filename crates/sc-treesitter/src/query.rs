use crate::grammar;

/// One named query asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct QueryInfo {
    /// Grammar id the query is written for.
    pub language: &'static str,
    /// Asset name without the `.scm` suffix (`--query`).
    pub name: &'static str,
    /// Asset bytes, including the trailing newline.
    ///
    /// Producers hash this text, with CRLF and bare CR normalized to LF, into
    /// `producer.query_sha256`. The packaged assets contain no CR, so the
    /// digest equals SHA-256 of these bytes.
    pub source: &'static str,
}

#[cfg(feature = "python")]
pub(crate) const PYTHON: &[QueryInfo] = &[QueryInfo {
    language: "python",
    name: "branches",
    source: include_str!("../assets/queries/python/branches.scm"),
}];

#[cfg(feature = "rust")]
pub(crate) const RUST: &[QueryInfo] = &[QueryInfo {
    language: "rust",
    name: "match-arms",
    source: include_str!("../assets/queries/rust/match-arms.scm"),
}];

/// Returns query assets for `language`, or every compiled-in language.
///
/// # Errors
///
/// Returns [`JevError::LanguageUnavailable`] when `language` is set and that
/// grammar is not compiled into this build.
pub fn queries(language: Option<&str>) -> Result<Vec<&'static QueryInfo>, JevError> {
    match language {
        Some(id) => Ok(grammar::lookup(id)?.queries.iter().collect()),
        None => Ok(grammar::all()
            .iter()
            .flat_map(|grammar| grammar.queries)
            .collect()),
    }
}

/// Returns the query text for `language` and `name`.
///
/// The returned bytes are the asset file, including its trailing newline.
/// Producers hash this text, with CRLF and bare CR normalized to LF, into
/// `producer.query_sha256`. The packaged assets contain no CR, so the digest
/// equals SHA-256 of these bytes.
///
/// # Errors
///
/// Returns [`JevError::LanguageUnavailable`] when the grammar is not compiled
/// in, and [`JevError::QueryNotFound`] when the asset name is missing.
pub fn query_source(language: &str, name: &str) -> Result<&'static str, JevError> {
    grammar::lookup(language)?
        .queries
        .iter()
        .find(|asset| asset.name == name)
        .map(|asset| asset.source)
        .ok_or_else(|| JevError::query_not_found(crate::PRODUCER_NAME, name))
}
