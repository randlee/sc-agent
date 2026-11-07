//! Tree-sitter producer for `sc/subjects/1` documents.
//!
//! `sc-treesitter` parses a source blob with a pinned grammar, runs a query in
//! matches mode, and returns a [`SubjectsDocument`]. The CLI reads blob bytes
//! with `git cat-file` and writes that document to stdout. This library does
//! not spawn git and does not depend on `sc-jev` or `sc-jev-check`.
//!
//! Normative JSON rules live in `docs/sc-agent-types/subjects-schema.md`.
//!
//! [`SubjectsDocument`]: sc_agent_types::schema::subjects::SubjectsDocument
//!
//! # Examples
//!
//! ```
//! # #[cfg(feature = "python")] {
//! let input = sc_treesitter::ExtractInput::new(
//!     "python",
//!     "(binary_operator left: (identifier) @name right: (identifier) @name)\n",
//!     b"value = left + right\n",
//! )
//! .with_path("sample.py")
//! .with_rev("HEAD")
//! .with_blob_sha("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
//! let document = sc_treesitter::extract(&input).expect("extract");
//! assert_eq!(document.schema(), sc_agent_types::schema::subjects::SUBJECTS_SCHEMA_V1);
//! assert_eq!(document.producer().mode, "matches");
//! assert_eq!(document.subjects().len(), 2);
//! # }
//! ```

#![warn(missing_docs)]

mod extract;
mod grammar;
mod query;
mod render;

#[doc(inline)]
pub use extract::ExtractInput;
#[doc(inline)]
pub use extract::extract;
#[doc(inline)]
pub use grammar::LanguageInfo;
#[doc(inline)]
pub use grammar::language;
#[doc(inline)]
pub use grammar::languages;
#[doc(inline)]
pub use query::QueryInfo;
#[doc(inline)]
pub use query::queries;
#[doc(inline)]
pub use query::query_source;
#[doc(inline)]
pub use render::subjects_to_json_bytes;

/// Producer name recorded in `producer.name`.
pub const PRODUCER_NAME: &str = "sc-treesitter";

/// Query mode recorded in `producer.mode`.
///
/// Phase A always runs tree-sitter matches. `--captures` filters names after
/// the match and does not change this value.
pub const PRODUCER_MODE: &str = "matches";
