//! `sc/subjects/1` document types.
//!
//! Normative JSON rules live in `docs/sc-agent-types/subjects-schema.md`.

use serde::{Deserialize, Serialize};

use crate::ids::Sha256Digest;

/// Schema id for the Phase A subjects artifact.
pub const SUBJECTS_SCHEMA_V1: &str = "sc/subjects/1";

/// Git identity of the source the subjects were extracted from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubjectsSource {
    /// Repo-relative source path.
    pub path: String,
    /// Git revision, or `HEAD` when the producer recorded that name.
    pub rev: String,
    /// Git blob id of the source bytes.
    pub blob_sha: String,
}

/// Tool that emitted the subjects document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubjectsProducer {
    /// Producer name, such as `sc-treesitter`.
    pub name: String,
    /// Producer version.
    pub version: String,
    /// Grammar language id.
    pub language: String,
    /// Grammar crate or package name.
    pub grammar: String,
    /// Pinned grammar version.
    pub grammar_version: String,
    /// SHA-256 of the query text that produced these subjects.
    pub query_sha256: Sha256Digest,
    /// Tree-sitter query mode. Phase A producers record `matches`.
    pub mode: String,
}

/// Half-open byte span and inclusive 1-based line span of a subject.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Span {
    /// First line, 1-based.
    pub start_line: u32,
    /// Last line, 1-based and inclusive.
    pub end_line: u32,
    /// First byte, 0-based.
    pub start_byte: u32,
    /// Byte one past the end of the subject text.
    pub end_byte: u32,
}

/// One extracted subject.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subject {
    /// `{path}:{start_line}` or `{path}:{start_line}:{n}` for later same-line hits.
    pub id: String,
    /// Capture name. Names starting with `@_` are not emitted.
    pub capture: String,
    /// UTF-8 slice `[start_byte, end_byte)` of the source.
    pub text: String,
    /// Location of `text` in the source.
    pub span: Span,
}

/// A `sc/subjects/1` document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubjectsDocument {
    /// Schema id. Phase A documents use [`SUBJECTS_SCHEMA_V1`].
    pub schema: String,
    /// Source identity.
    pub source: SubjectsSource,
    /// Producer identity.
    pub producer: SubjectsProducer,
    /// Subjects sorted by `(start_byte, capture)`.
    pub subjects: Vec<Subject>,
}

/// Named check input.
///
/// Only the four Phase A kinds exist. `sc-jev-check` is the only crate that
/// runs them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum InputSpec {
    /// Read a file at `rev`.
    File {
        /// Input name in `check.toml`.
        name: String,
        /// Repo-relative path.
        path: String,
    },
    /// Read one markdown table column.
    TableColumn {
        /// Input name in `check.toml`.
        name: String,
        /// Repo-relative path.
        path: String,
        /// Column header to extract.
        column: String,
    },
    /// Read a `sc/subjects/1` document from `path`.
    Subjects {
        /// Input name in `check.toml`.
        name: String,
        /// Repo-relative path of the artifact.
        path: String,
    },
    /// Run an argv array. No shell.
    Command {
        /// Input name in `check.toml`.
        name: String,
        /// Argument vector. Placeholders are expanded by the facade.
        argv: Vec<String>,
        /// Process time limit in seconds.
        timeout_seconds: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_fixture_is_schema_v1_with_a_same_line_id() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/jev/subjects-schema/v1/minimal.json");
        let text = std::fs::read_to_string(path).expect("read minimal.json");
        let document: SubjectsDocument = serde_json::from_str(&text).expect("parse subjects");
        assert_eq!(document.schema, SUBJECTS_SCHEMA_V1);
        assert_eq!(document.producer.mode, "matches");
        assert_eq!(document.subjects[0].id, "src/example.py:42");
        assert_eq!(document.subjects[1].id, "src/example.py:42:1");
        assert!(document.subjects[0].span.start_byte < document.subjects[1].span.start_byte);
    }
}
