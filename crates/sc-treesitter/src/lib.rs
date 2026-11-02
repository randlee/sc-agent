//! Tree-sitter producer for `sc/subjects/1` documents.
//!
//! `sc-treesitter` parses a source blob with a pinned grammar, runs a query in
//! matches mode, and returns a [`SubjectsDocument`]. The CLI reads bytes with
//! `git show` and writes that document to stdout. This library does not spawn
//! git and does not depend on `sc-jev` or `sc-jev-check`.
//!
//! Normative JSON rules live in `docs/sc-agent-types/subjects-schema.md`.
//!
//! # Examples
//!
//! ```
//! let source = b"value = left + right\n";
//! let document = sc_treesitter::extract(&sc_treesitter::ExtractInput {
//!     language: "python",
//!     query: "(binary_operator left: (identifier) @name right: (identifier) @name)\n",
//!     captures: None,
//!     source,
//!     path: "sample.py",
//!     rev: "HEAD",
//!     blob_sha: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
//! })
//! .expect("extract");
//! assert_eq!(document.schema, sc_agent_types::SUBJECTS_SCHEMA_V1);
//! assert_eq!(document.producer.mode, "matches");
//! assert_eq!(document.subjects.len(), 2);
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
