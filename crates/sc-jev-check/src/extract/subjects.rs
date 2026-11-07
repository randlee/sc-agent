//! `subjects` extractor.
//!
//! Reads a `sc/subjects/1` document from a repo-relative path. The engine
//! never reads process stdin: `-` is refused with a pointer to
//! `--request-json -` or a committed file.

use sc_agent_types::{
    InputSpec, JevError, RepoRoot, Rev, SUBJECTS_SCHEMA_V1, Subject, SubjectsDocument,
};

use super::Extracted;
use super::Extractor;
use super::private::Sealed;
use super::read::read_repo_bytes;

/// Validates `sc/subjects/1` and returns its subjects.
#[derive(Debug, Clone, Copy)]
pub struct SubjectsExtractor;

impl Sealed for SubjectsExtractor {}

impl Extractor for SubjectsExtractor {
    fn kind(&self) -> &'static str {
        "subjects"
    }

    fn extract(
        &self,
        repo: &RepoRoot,
        rev: &Rev,
        input: &InputSpec,
    ) -> Result<Vec<Subject>, JevError> {
        Ok(extract_subjects(repo, rev, input)?.subjects)
    }
}

pub(crate) fn extract_subjects(
    repo: &RepoRoot,
    rev: &Rev,
    input: &InputSpec,
) -> Result<Extracted, JevError> {
    let InputSpec::Subjects { path } = input else {
        return Err(JevError::ConfigInvalid {
            detail: "subjects extractor received a different input kind".into(),
        });
    };
    if path == "-" {
        return Err(JevError::ConfigInvalid {
            detail: "subjects input path - (stdin) is not supported; pass a caller-built document with sc-ai jev run --request-json -, or commit the subjects.json file and name its repo-relative path".into(),
        });
    }
    let bytes = read_repo_bytes(repo, rev, path)?;
    parse_subjects_bytes(&bytes)
}

/// Parses and validates a `sc/subjects/1` document.
///
/// # Errors
///
/// Returns [`JevError::ExtractionInvalid`] when the bytes are not JSON or
/// the document is not the schema, and [`JevError::ExtractionEmpty`] when
/// `subjects` is empty.
pub fn parse_subjects_bytes(bytes: &[u8]) -> Result<Extracted, JevError> {
    let document: SubjectsDocument =
        serde_json::from_slice(bytes).map_err(|error| JevError::ExtractionInvalid {
            detail: format!("subjects JSON: {error}"),
        })?;
    validate_subjects_document(&document)?;
    Ok(Extracted {
        subjects: document.subjects.clone(),
        document: Some(document),
    })
}

/// Checks schema id, id rules, spans, sort order, and the helper-capture rule.
///
/// A capture name beginning with `_` (a query helper, stored without `@`)
/// must not appear.
///
/// # Errors
///
/// Returns [`JevError::ExtractionEmpty`] or [`JevError::ExtractionInvalid`].
pub fn validate_subjects_document(document: &SubjectsDocument) -> Result<(), JevError> {
    if document.schema != SUBJECTS_SCHEMA_V1 {
        return Err(JevError::ExtractionInvalid {
            detail: format!(
                "schema is {}, expected {SUBJECTS_SCHEMA_V1}",
                document.schema
            ),
        });
    }
    if document.subjects.is_empty() {
        return Err(JevError::ExtractionEmpty);
    }
    if document.source.path.is_empty() || document.producer.mode.is_empty() {
        return Err(JevError::ExtractionInvalid {
            detail: "subjects source path and producer mode must be non-empty".into(),
        });
    }
    let mut previous: Option<(u32, &str)> = None;
    let mut line_counts: Vec<(u32, u32)> = Vec::new();
    for subject in &document.subjects {
        // Capture names are stored without `@`; query helpers (`@_name`)
        // arrive as `_name`. Producers drop them, so one here is a leak.
        if subject.capture.trim_start_matches('@').starts_with('_') {
            return Err(JevError::ExtractionInvalid {
                detail: format!("helper capture {} must not be emitted", subject.capture),
            });
        }
        if subject.span.start_line == 0 || subject.span.end_line < subject.span.start_line {
            return Err(JevError::ExtractionInvalid {
                detail: format!("span lines are invalid for {}", subject.id),
            });
        }
        if subject.span.end_byte < subject.span.start_byte {
            return Err(JevError::ExtractionInvalid {
                detail: format!("span bytes are invalid for {}", subject.id),
            });
        }
        let width = subject.span.end_byte - subject.span.start_byte;
        if usize::try_from(width).ok() != Some(subject.text.len()) {
            return Err(JevError::ExtractionInvalid {
                detail: format!("text length does not match the span for {}", subject.id),
            });
        }
        let key = (subject.span.start_byte, subject.capture.as_str());
        if let Some(prev) = previous
            && key < prev
        {
            return Err(JevError::ExtractionInvalid {
                detail: "subjects are not sorted by (start_byte, capture)".into(),
            });
        }
        previous = Some(key);
        let count = line_counts
            .iter()
            .find(|(line, _)| *line == subject.span.start_line)
            .map(|(_, count)| *count)
            .unwrap_or(0);
        let expected = if count == 0 {
            format!("{}:{}", document.source.path, subject.span.start_line)
        } else {
            format!(
                "{}:{}:{count}",
                document.source.path, subject.span.start_line
            )
        };
        if subject.id != expected {
            return Err(JevError::ExtractionInvalid {
                detail: format!("subject id is {}, expected {expected}", subject.id),
            });
        }
        if let Some((_, seen)) = line_counts
            .iter_mut()
            .find(|(line, _)| *line == subject.span.start_line)
        {
            *seen = seen.saturating_add(1);
        } else {
            line_counts.push((subject.span.start_line, 1));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::test_support::workspace_root;
    use sc_agent_types::{RepoRoot, Rev};

    fn minimal_value() -> serde_json::Value {
        let path = workspace_root().join("tests/fixtures/jev/subjects-schema/v1/minimal.json");
        let text = std::fs::read_to_string(path).expect("minimal.json");
        serde_json::from_str(&text).expect("json")
    }

    #[test]
    fn subjects_minimal_fixture_maps_ids_and_text() {
        let repo = RepoRoot::new(workspace_root());
        let extracted = extract_subjects(
            &repo,
            &Rev::worktree(),
            &InputSpec::Subjects {
                path: "tests/fixtures/jev/subjects-schema/v1/minimal.json".into(),
            },
        )
        .expect("subjects");
        assert_eq!(extracted.subjects[0].id, "src/example.py:42");
        assert_eq!(extracted.subjects[1].id, "src/example.py:42:1");
        assert_eq!(extracted.subjects[0].text, "alpha");
        assert_eq!(extracted.subjects[1].text, "beta");
        let document = extracted.document.expect("document");
        assert_eq!(document.schema, SUBJECTS_SCHEMA_V1);
        assert_eq!(document.producer.mode, "matches");
        assert_eq!(
            SubjectsExtractor
                .extract(
                    &repo,
                    &Rev::worktree(),
                    &InputSpec::Subjects {
                        path: "tests/fixtures/jev/subjects-schema/v1/minimal.json".into(),
                    },
                )
                .expect("trait")
                .len(),
            2
        );
    }

    #[test]
    fn subjects_stdin_path_is_config_invalid() {
        let repo = RepoRoot::new(workspace_root());
        let error = extract_subjects(
            &repo,
            &Rev::worktree(),
            &InputSpec::Subjects { path: "-".into() },
        )
        .expect_err("stdin");
        assert_eq!(error.code(), "CONFIG_INVALID");
        assert!(error.fix_hint().contains("--request-json -"));
    }

    #[test]
    fn subjects_rejects_wrong_schema() {
        let mut value = minimal_value();
        value["schema"] = serde_json::json!("sc/subjects/0");
        let error =
            parse_subjects_bytes(&serde_json::to_vec(&value).expect("bytes")).expect_err("schema");
        assert_eq!(error.code(), "EXTRACTION_INVALID");
    }

    #[test]
    fn subjects_empty_is_extraction_empty() {
        let mut value = minimal_value();
        value["subjects"] = serde_json::json!([]);
        let error =
            parse_subjects_bytes(&serde_json::to_vec(&value).expect("bytes")).expect_err("empty");
        assert_eq!(error.code(), "EXTRACTION_EMPTY");
    }

    #[test]
    fn subjects_helper_capture_is_rejected() {
        let mut value = minimal_value();
        for helper in ["_skip", "@_skip"] {
            value["subjects"][0]["capture"] = serde_json::json!(helper);
            let error = parse_subjects_bytes(&serde_json::to_vec(&value).expect("bytes"))
                .expect_err("helper");
            assert_eq!(error.code(), "EXTRACTION_INVALID", "{helper}");
        }
    }

    #[test]
    fn subjects_unsorted_is_rejected() {
        let mut value = minimal_value();
        value["subjects"][0]["span"]["start_byte"] = serde_json::json!(30);
        value["subjects"][0]["span"]["end_byte"] = serde_json::json!(35);
        value["subjects"][1]["span"]["start_byte"] = serde_json::json!(10);
        value["subjects"][1]["span"]["end_byte"] = serde_json::json!(14);
        let error =
            parse_subjects_bytes(&serde_json::to_vec(&value).expect("bytes")).expect_err("order");
        assert_eq!(error.code(), "EXTRACTION_INVALID");
        assert!(error.to_string().contains("sorted"));
    }
}
