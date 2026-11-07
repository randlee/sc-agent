//! `sc/subjects/1` document types.
//!
//! Normative JSON rules live in `docs/sc-agent-types/subjects-schema.md`.

use serde::{Deserialize, Serialize};

use crate::ids::{Rev, Sha256Digest};

/// Schema id for the Phase A subjects artifact.
pub const SUBJECTS_SCHEMA_V1: &str = "sc/subjects/1";

/// Git identity of the source the subjects were extracted from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubjectsSource {
    /// Repo-relative source path.
    pub path: String,
    /// Git revision, or `HEAD` when the producer recorded that name.
    pub rev: Rev,
    /// Git blob id of the source bytes.
    ///
    /// Plain `String` by decision (a-10, RBP-F011): a validating `BlobSha`
    /// newtype would tighten what `sc/subjects/1` documents from command
    /// extractors deserialize to, which is the same contract question as
    /// `Rev` deserialize validation and is deferred to Phase A.1 r-1.
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
    /// Capture name, stored without `@`. Names starting with `_` (query
    /// helper captures `@_…`) are not emitted.
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

/// Check input stored under `[inputs.<name>]` in `check.toml`.
///
/// `<name>` is the table key, not a field. Keys match
/// `docs/sc-jev-check/extraction.md`: `kind`, `path`, `column`, and for
/// `command` the argv array `command` plus optional `timeout_seconds`
/// (default 30). Only these four Phase A kinds exist. `sc-jev-check` is the
/// only crate that runs them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum InputSpec {
    /// Read a file at the caller-supplied revision.
    File {
        /// Repo-relative path.
        path: String,
    },
    /// Read one markdown table column.
    TableColumn {
        /// Repo-relative path.
        path: String,
        /// Column header to extract.
        column: String,
    },
    /// Read a `sc/subjects/1` document from `path`.
    Subjects {
        /// Repo-relative path of the artifact.
        path: String,
    },
    /// Run an argv array. No shell.
    Command {
        /// Argument vector. The check.toml key is `command`.
        command: Vec<String>,
        /// Process time limit in seconds. Omitted toml uses 30.
        #[serde(default = "default_command_timeout_seconds")]
        timeout_seconds: u64,
    },
}

fn default_command_timeout_seconds() -> u64 {
    30
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
        assert_eq!(document.source.rev, Rev::new("HEAD"));
        assert_eq!(document.producer.mode, "matches");
        assert_eq!(document.subjects[0].id, "src/example.py:42");
        assert_eq!(document.subjects[1].id, "src/example.py:42:1");
        assert!(document.subjects[0].span.start_byte < document.subjects[1].span.start_byte);
    }

    #[test]
    fn input_spec_matches_extraction_check_toml_keys() {
        let file: InputSpec = serde_json::from_str(
            r#"{"kind":"file","path":"skills/atm-bd-lite/scripts/atm-bd-task-hook.py"}"#,
        )
        .expect("file");
        assert_eq!(
            file,
            InputSpec::File {
                path: "skills/atm-bd-lite/scripts/atm-bd-task-hook.py".into(),
            }
        );

        let subjects: InputSpec = serde_json::from_str(
            r#"{"kind":"subjects","path":"tests/fixtures/cross-impl/branches/subjects.json"}"#,
        )
        .expect("subjects");
        assert!(matches!(subjects, InputSpec::Subjects { .. }));

        let column: InputSpec = serde_json::from_str(
            r#"{"kind":"table-column","path":"docs/decisions.md","column":"Choice"}"#,
        )
        .expect("table-column");
        assert_eq!(
            column,
            InputSpec::TableColumn {
                path: "docs/decisions.md".into(),
                column: "Choice".into(),
            }
        );

        let command: InputSpec = serde_json::from_str(
            r#"{"kind":"command","command":["sc-treesitter","extract","--language","python","--query","branches","{path}"]}"#,
        )
        .expect("command");
        assert_eq!(
            command,
            InputSpec::Command {
                command: vec![
                    "sc-treesitter".into(),
                    "extract".into(),
                    "--language".into(),
                    "python".into(),
                    "--query".into(),
                    "branches".into(),
                    "{path}".into(),
                ],
                timeout_seconds: 30,
            }
        );

        let with_name =
            serde_json::from_str::<InputSpec>(r#"{"kind":"file","name":"hook","path":"a.py"}"#);
        assert!(
            with_name.is_err(),
            "input name is the table key, not a field"
        );
        let with_argv = serde_json::from_str::<InputSpec>(r#"{"kind":"command","argv":["echo"]}"#);
        assert!(with_argv.is_err(), "argv is the check.toml key command");
    }
}
