//! Matches-mode extraction into `sc/subjects/1`.

use std::collections::BTreeMap;

use sc_agent_types::{
    JevError, Rev, SUBJECTS_SCHEMA_V1, Sha256Digest, Span, Subject, SubjectsDocument,
    SubjectsProducer, SubjectsSource,
};
use tree_sitter::{Node, Parser, Point, Query, QueryCursor, StreamingIterator};

use crate::grammar;
use crate::{PRODUCER_MODE, PRODUCER_NAME};

/// Source and query for one extraction.
///
/// The caller supplies the bytes. The library does not read git or the filesystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtractInput<'a> {
    /// Grammar id, such as `python` or `rust`.
    pub language: &'a str,
    /// Query text. `producer.query_sha256` is the SHA-256 of these bytes.
    pub query: &'a str,
    /// Capture allow-list.
    ///
    /// `None` emits every capture whose name does not start with `_`.
    /// `Some` emits only listed names. Helper captures (`@_…`, stored without
    /// the `@`) are never subjects.
    pub captures: Option<&'a [&'a str]>,
    /// UTF-8 source the query runs on.
    pub source: &'a [u8],
    /// Repo-relative path used in `source.path` and subject ids.
    pub path: &'a str,
    /// Revision name stored in `source.rev`, such as `HEAD`.
    pub rev: &'a str,
    /// Git blob id of `source`.
    pub blob_sha: &'a str,
}

struct RawCapture<'a> {
    name: &'a str,
    start_byte: usize,
    end_byte: usize,
    start: Point,
    end: Point,
}

/// Runs `input.query` in matches mode and builds a subjects document.
///
/// Subjects are ordered by `(start_byte, capture)`. The first subject on a
/// line is `{path}:{line}`. Later subjects on that line are `{path}:{line}:1`,
/// `{path}:{line}:2`, and so on. `producer.mode` is always [`PRODUCER_MODE`].
///
/// # Errors
///
/// Returns [`JevError::LanguageUnavailable`] for an unknown grammar,
/// [`JevError::QueryInvalid`] when the query does not compile,
/// [`JevError::ExtractionParseError`] when an `ERROR` node sits inside a
/// subject span, [`JevError::ExtractionEmpty`] when no subject remains,
/// and [`JevError::InputInvalid`] when the source is not UTF-8 or a required
/// field is empty.
///
/// # Examples
///
/// ```
/// let document = sc_treesitter::extract(&sc_treesitter::ExtractInput {
///     language: "python",
///     query: "(identifier) @name\n",
///     captures: Some(&["name"]),
///     source: b"value = 1\n",
///     path: "sample.py",
///     rev: "HEAD",
///     blob_sha: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
/// })
/// .expect("identifier");
/// assert_eq!(document.subjects[0].capture, "name");
/// assert_eq!(document.subjects[0].text, "value");
/// ```
pub fn extract(input: &ExtractInput<'_>) -> Result<SubjectsDocument, JevError> {
    if input.path.is_empty() {
        return Err(JevError::InputInvalid {
            detail: "source path is empty".into(),
        });
    }
    if input.rev.is_empty() {
        return Err(JevError::InputInvalid {
            detail: "revision is empty".into(),
        });
    }
    if input.blob_sha.is_empty() {
        return Err(JevError::InputInvalid {
            detail: "blob sha is empty".into(),
        });
    }
    if input.query.is_empty() {
        return Err(JevError::QueryInvalid {
            detail: "query text is empty".into(),
        });
    }
    if std::str::from_utf8(input.source).is_err() {
        return Err(JevError::InputInvalid {
            detail: "source is not UTF-8".into(),
        });
    }

    let info = grammar::language(input.language)?;
    let language = grammar::load(input.language)?;
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .map_err(|error| JevError::ExtractionFailed {
            detail: format!("grammar for {} failed to load: {error}", info.id),
        })?;
    let query = Query::new(&language, input.query).map_err(|error| JevError::QueryInvalid {
        detail: error.to_string(),
    })?;
    let tree = parser
        .parse(input.source, None)
        .ok_or_else(|| JevError::ExtractionFailed {
            detail: "parser returned no tree".into(),
        })?;

    let mut found = Vec::new();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), input.source);
    while let Some(query_match) = matches.next() {
        for capture in query_match.captures() {
            let Some(name) = query.capture_names().get(capture.index as usize).copied() else {
                return Err(JevError::ExtractionFailed {
                    detail: "capture index was outside the query".into(),
                });
            };
            if !capture_allowed(name, input.captures) {
                continue;
            }
            found.push(RawCapture {
                name,
                start_byte: capture.node.start_byte(),
                end_byte: capture.node.end_byte(),
                start: capture.node.start_position(),
                end: capture.node.end_position(),
            });
        }
    }
    drop(matches);

    if found.is_empty() {
        return Err(JevError::ExtractionEmpty);
    }

    found.sort_by(|left, right| {
        left.start_byte
            .cmp(&right.start_byte)
            .then_with(|| left.name.cmp(right.name))
            .then_with(|| left.end_byte.cmp(&right.end_byte))
    });

    let root = tree.root_node();
    let mut parse_error_line = None;
    for capture in &found {
        if let Some(line) = error_line_inside(root, capture.start_byte, capture.end_byte) {
            parse_error_line =
                Some(parse_error_line.map_or(line, |previous: u32| previous.min(line)));
        }
    }
    if let Some(line) = parse_error_line {
        return Err(JevError::ExtractionParseError {
            path: input.path.to_owned(),
            line,
        });
    }

    let mut subjects = Vec::with_capacity(found.len());
    let mut line_counts: BTreeMap<u32, u32> = BTreeMap::new();
    for capture in found {
        if capture.end_byte < capture.start_byte || capture.end_byte > input.source.len() {
            return Err(JevError::ExtractionFailed {
                detail: format!(
                    "capture span {}..{} is outside the source",
                    capture.start_byte, capture.end_byte
                ),
            });
        }
        let text = std::str::from_utf8(&input.source[capture.start_byte..capture.end_byte])
            .map_err(|_| JevError::ExtractionInvalid {
                detail: format!(
                    "subject bytes {}..{} are not UTF-8",
                    capture.start_byte, capture.end_byte
                ),
            })?;
        let (start_line, end_line) = inclusive_lines(capture.start, capture.end)?;
        let index_on_line = line_counts.entry(start_line).or_insert(0);
        let id = subject_id(input.path, start_line, *index_on_line);
        *index_on_line = index_on_line.saturating_add(1);
        subjects.push(Subject {
            id,
            capture: capture.name.to_owned(),
            text: text.to_owned(),
            span: Span {
                start_line,
                end_line,
                start_byte: u32_from_usize(capture.start_byte)?,
                end_byte: u32_from_usize(capture.end_byte)?,
            },
        });
    }

    Ok(SubjectsDocument {
        schema: SUBJECTS_SCHEMA_V1.to_owned(),
        source: SubjectsSource {
            path: input.path.to_owned(),
            rev: Rev::new(input.rev),
            blob_sha: input.blob_sha.to_owned(),
        },
        producer: SubjectsProducer {
            name: PRODUCER_NAME.to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            language: info.id.to_owned(),
            grammar: info.grammar.to_owned(),
            grammar_version: info.grammar_version.to_owned(),
            query_sha256: Sha256Digest::of(input.query.as_bytes()),
            mode: PRODUCER_MODE.to_owned(),
        },
        subjects,
    })
}

fn capture_allowed(name: &str, captures: Option<&[&str]>) -> bool {
    if name.starts_with('_') {
        return false;
    }
    match captures {
        Some(allow) => allow.contains(&name),
        None => true,
    }
}

/// First subject on a line has no suffix. Later ones use `:1`, `:2`, …
fn subject_id(path: &str, line: u32, index_on_line: u32) -> String {
    if index_on_line == 0 {
        format!("{path}:{line}")
    } else {
        format!("{path}:{line}:{index_on_line}")
    }
}

fn inclusive_lines(start: Point, end: Point) -> Result<(u32, u32), JevError> {
    let start_line = u32_from_usize(start.row.saturating_add(1))?;
    let end_row = if end.column == 0 && end.row > start.row {
        end.row
    } else {
        end.row.saturating_add(1)
    };
    let end_line = u32_from_usize(end_row.max(start_line as usize))?;
    Ok((start_line, end_line))
}

fn u32_from_usize(value: usize) -> Result<u32, JevError> {
    u32::try_from(value).map_err(|_| JevError::InputTooLarge {
        limit_bytes: u64::from(u32::MAX),
        actual_bytes: u64::try_from(value).unwrap_or(u64::MAX),
    })
}

fn error_line_inside(root: Node<'_>, start: usize, end: usize) -> Option<u32> {
    let mut found = None;
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        let node_start = node.start_byte();
        let node_end = node.end_byte();
        if node_end < start || node_start > end {
            continue;
        }
        if node.is_error() && node_start >= start && node_start < end && node_end <= end {
            let line =
                u32::try_from(node.start_position().row.saturating_add(1)).unwrap_or(u32::MAX);
            found = Some(found.map_or(line, |previous: u32| previous.min(line)));
        }
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                stack.push(cursor.node());
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query_source;
    use crate::subjects_to_json_bytes;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    const BLOB: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn run<'a>(
        language: &'a str,
        query: &'a str,
        source: &'a [u8],
        path: &'a str,
        captures: Option<&'a [&'a str]>,
    ) -> Result<SubjectsDocument, JevError> {
        extract(&ExtractInput {
            language,
            query,
            captures,
            source,
            path,
            rev: "HEAD",
            blob_sha: BLOB,
        })
    }

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/cross-impl/branches")
            .join(name)
    }

    #[test]
    fn test_tree_sitter_python_branches_lists_arms_and_refusals() {
        let source = std::fs::read(fixture("hook.py")).expect("hook.py");
        let query = query_source("python", "branches").expect("branches query");
        let document = run(
            "python",
            query,
            &source,
            "tests/fixtures/cross-impl/branches/hook.py",
            None,
        )
        .expect("extract branches");
        assert_eq!(document.schema, SUBJECTS_SCHEMA_V1);
        assert_eq!(document.producer.mode, PRODUCER_MODE);
        assert_eq!(document.producer.name, PRODUCER_NAME);
        assert_eq!(document.producer.grammar, "tree-sitter-python");
        assert_eq!(document.producer.grammar_version, "0.25.0");
        let arms: Vec<&str> = document
            .subjects
            .iter()
            .filter(|subject| subject.capture == "arm")
            .map(|subject| subject.text.as_str())
            .collect();
        let refusals: Vec<&str> = document
            .subjects
            .iter()
            .filter(|subject| subject.capture == "refusal")
            .map(|subject| subject.text.as_str())
            .collect();
        assert_eq!(
            arms,
            vec![
                "Transition.GIVE_BACK",
                "Transition.KEEP",
                "Transition.STOP",
                "_"
            ]
        );
        assert_eq!(
            refusals,
            vec!["Refusal(\"give-back\")", "Refusal(\"stop\")"]
        );
        let only_arms = run(
            "python",
            query,
            &source,
            "tests/fixtures/cross-impl/branches/hook.py",
            Some(&["arm"]),
        )
        .expect("arm allow-list");
        assert!(
            only_arms
                .subjects
                .iter()
                .all(|subject| subject.capture == "arm")
        );
        assert_eq!(only_arms.producer.mode, PRODUCER_MODE);
    }

    #[test]
    fn test_tree_sitter_rust_match_arms() {
        let source = r#"
fn classify(value: Transition) -> &'static str {
    match value {
        Transition::GiveBack => "give-back",
        Transition::Keep => "keep",
    }
}
"#;
        let query = query_source("rust", "match-arms").expect("match-arms");
        let document =
            run("rust", query, source.as_bytes(), "src/classify.rs", None).expect("rust");
        let arms: Vec<&str> = document
            .subjects
            .iter()
            .map(|subject| subject.text.as_str())
            .collect();
        assert_eq!(arms, vec!["Transition::GiveBack", "Transition::Keep"]);
        assert!(
            document
                .subjects
                .iter()
                .all(|subject| subject.capture == "arm")
        );
        assert_eq!(document.producer.mode, PRODUCER_MODE);
        assert_eq!(document.producer.grammar, "tree-sitter-rust");
        assert_eq!(document.producer.grammar_version, "0.24.2");
    }

    #[test]
    fn test_tree_sitter_helper_capture_is_not_a_subject() {
        let source = std::fs::read(fixture("hook.py")).expect("hook.py");
        let query = query_source("python", "branches").expect("branches query");
        assert!(query.contains("@_fn"), "fixture query must name a helper");
        let document = run("python", query, &source, "hook.py", None).expect("extract");
        assert!(
            document
                .subjects
                .iter()
                .all(|subject| !subject.capture.starts_with('_'))
        );
        assert!(
            document
                .subjects
                .iter()
                .all(|subject| subject.capture != "_fn" && subject.text != "Refusal")
        );
        assert!(
            document
                .subjects
                .iter()
                .any(|subject| subject.capture == "refusal" && subject.text.contains("Refusal"))
        );
    }

    #[test]
    fn test_tree_sitter_same_line_ids_are_unique_and_stable() {
        let source = b"return Refusal(\"give-back\")\n";
        let query = "(return_statement (call function: (identifier) @arm arguments: (argument_list (string) @refusal)))\n";
        let once = run("python", query, source, "same.py", None).expect("once");
        let twice = run("python", query, source, "same.py", None).expect("twice");
        assert_eq!(once, twice);
        assert_eq!(once.subjects.len(), 2);
        assert_eq!(
            once.subjects[0].span.start_line,
            once.subjects[1].span.start_line
        );
        assert_eq!(once.subjects[0].id, "same.py:1");
        assert_eq!(once.subjects[1].id, "same.py:1:1");
        assert!(once.subjects[0].span.start_byte < once.subjects[1].span.start_byte);
        let ids: BTreeSet<&str> = once
            .subjects
            .iter()
            .map(|subject| subject.id.as_str())
            .collect();
        assert_eq!(ids.len(), once.subjects.len());
    }

    #[test]
    fn test_tree_sitter_parse_error_is_typed() {
        let error = run("python", "(ERROR) @broken\n", b"def (\n", "broken.py", None)
            .expect_err("syntax error must fail");
        assert_eq!(error.code(), "EXTRACTION_PARSE_ERROR");
        match error {
            JevError::ExtractionParseError { path, line } => {
                assert_eq!(path, "broken.py");
                assert!(line >= 1, "line {line}");
            }
            other => panic!("unexpected {}", other.code()),
        }
    }

    #[test]
    fn test_tree_sitter_unknown_language_is_typed() {
        let error = run("cobol", "(identifier) @name\n", b"x\n", "a.py", None).expect_err("cobol");
        assert_eq!(error.code(), "LANGUAGE_UNAVAILABLE");
        match error {
            JevError::LanguageUnavailable { language } => assert_eq!(language, "cobol"),
            other => panic!("unexpected {}", other.code()),
        }
    }

    #[test]
    fn test_matches_mode_duplicate_captures() {
        let query = "(binary_operator left: (identifier) @name right: (identifier) @name)\n";
        let document =
            run("python", query, b"value = left + right\n", "dup.py", None).expect("dup");
        assert_eq!(document.producer.mode, "matches");
        let names: Vec<&str> = document
            .subjects
            .iter()
            .filter(|subject| subject.capture == "name")
            .map(|subject| subject.text.as_str())
            .collect();
        assert_eq!(names, vec!["left", "right"]);
    }

    #[test]
    fn test_subjects_schema_roundtrip() {
        let query = query_source("python", "branches").expect("query");
        let source = std::fs::read(fixture("hook.py")).expect("hook.py");
        let document = run(
            "python",
            query,
            &source,
            "tests/fixtures/cross-impl/branches/hook.py",
            None,
        )
        .expect("extract");
        let bytes = subjects_to_json_bytes(&document).expect("json");
        let parsed: SubjectsDocument = serde_json::from_slice(&bytes).expect("parse");
        assert_eq!(parsed, document);
        assert_eq!(parsed.schema, SUBJECTS_SCHEMA_V1);
        assert_eq!(subjects_to_json_bytes(&parsed).expect("again"), bytes);
        let value: serde_json::Value = serde_json::from_slice(&bytes).expect("value");
        assert_eq!(value["schema"], "sc/subjects/1");
        assert_eq!(value["producer"]["mode"], "matches");
    }

    #[test]
    fn empty_extraction_is_typed() {
        let error = run(
            "python",
            "(class_definition) @name\n",
            b"pass\n",
            "empty.py",
            None,
        )
        .expect_err("no class");
        assert_eq!(error.code(), "EXTRACTION_EMPTY");
    }

    #[test]
    fn invalid_query_is_typed() {
        let error = run("python", "((( \n", b"pass\n", "bad.py", None).expect_err("query");
        assert_eq!(error.code(), "QUERY_INVALID");
    }

    #[test]
    fn python_branches_asset_matches_fixture_query() {
        let fixture = std::fs::read_to_string(fixture("queries/python-branches.scm")).expect("scm");
        assert_eq!(fixture, query_source("python", "branches").expect("asset"));
    }

    #[test]
    fn pinned_grammar_versions_match_cargo_toml() {
        let manifest = include_str!("../Cargo.toml");
        for language in crate::languages() {
            let needle = format!(
                "{} = {{ version = \"={}\"",
                language.grammar, language.grammar_version
            );
            assert!(manifest.contains(&needle), "missing {needle}");
        }
        assert!(manifest.contains("tree-sitter = { version = \"=0.27.1\""));
    }

    #[test]
    fn crate_manifest_does_not_depend_on_jev() {
        let manifest = include_str!("../Cargo.toml");
        assert!(!manifest.contains("sc-jev"));
    }
}
