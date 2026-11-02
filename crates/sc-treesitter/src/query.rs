//! Query assets compiled into the binary.
//!
//! `--language python --query branches` loads
//! `assets/queries/python/branches.scm`.

use sc_agent_types::JevError;

use crate::grammar;

/// One named query asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueryInfo {
    /// Grammar id the query is written for.
    pub language: &'static str,
    /// Asset name without the `.scm` suffix (`--query`).
    pub name: &'static str,
}

struct Asset {
    language: &'static str,
    name: &'static str,
    source: &'static str,
}

const ASSETS: &[Asset] = &[
    Asset {
        language: "python",
        name: "branches",
        source: include_str!("../assets/queries/python/branches.scm"),
    },
    Asset {
        language: "rust",
        name: "match-arms",
        source: include_str!("../assets/queries/rust/match-arms.scm"),
    },
];

/// Returns query assets for `language`, or every compiled-in language.
///
/// # Errors
///
/// Returns [`JevError::LanguageUnavailable`] when `language` is set and that
/// grammar is not compiled into this build.
pub fn queries(language: Option<&str>) -> Result<Vec<QueryInfo>, JevError> {
    if let Some(language) = language {
        grammar::language(language)?;
    }
    Ok(ASSETS
        .iter()
        .filter(|asset| language.is_none_or(|id| asset.language == id))
        .filter(|asset| grammar::language(asset.language).is_ok())
        .map(|asset| QueryInfo {
            language: asset.language,
            name: asset.name,
        })
        .collect())
}

/// Returns the query text for `language` and `name`.
///
/// The returned bytes are the asset file, including its trailing newline.
/// Producers hash these exact bytes into `producer.query_sha256`.
///
/// # Errors
///
/// Returns [`JevError::LanguageUnavailable`] when the grammar is not compiled
/// in, and [`JevError::QueryNotFound`] when the asset name is missing.
pub fn query_source(language: &str, name: &str) -> Result<&'static str, JevError> {
    grammar::language(language)?;
    ASSETS
        .iter()
        .find(|asset| asset.language == language && asset.name == name)
        .map(|asset| asset.source)
        .ok_or_else(|| JevError::QueryNotFound {
            query: name.to_owned(),
        })
}
