//! `subjects` extractor.
//!
//! Reads a `sc/subjects/1` document from a repo-relative path. The engine
//! never reads process stdin: `-` is refused with a pointer to
//! `--request-json -` or a committed file.

#[cfg(test)]
use sc_agent_types::schema::subjects::SUBJECTS_SCHEMA_V1;
use sc_agent_types::schema::subjects::SubjectsDocument;
use sc_agent_types::{JevError, RepoRelativePath, RepoRoot, Rev};

use super::Extracted;
use super::read::read_repo_bytes;

/// Validates the `sc/subjects/1` document at the input path.
pub(crate) fn extract_subjects(
    repo: &RepoRoot,
    rev: &Rev,
    path: &RepoRelativePath,
) -> Result<Extracted, JevError> {
    if path.as_str() == "-" {
        return Err(JevError::config_invalid(
            "subjects input path - (stdin) is not supported; pass a caller-built document with sc-ai jev run --request-json -, or commit the subjects.json file and name its repo-relative path",
        ));
    }
    let bytes = read_repo_bytes(repo, rev, path)?;
    parse_subjects_bytes(&bytes)
}

/// Parses and validates a `sc/subjects/1` document.
///
/// Validation is [`SubjectsDocument`]'s own (schema id, id rule, spans, sort
/// order, helper captures); this crate keeps no copy of it.
///
/// # Errors
///
/// Returns [`JevError::ExtractionInvalid`] when the bytes are not JSON or
/// the document is not the schema, and [`JevError::ExtractionEmpty`] when
/// `subjects` is empty.
pub(crate) fn parse_subjects_bytes(bytes: &[u8]) -> Result<Extracted, JevError> {
    let document = SubjectsDocument::from_json_slice(bytes)?;
    Ok(Extracted {
        subjects: document.subjects().to_vec(),
        document: Some(document),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::test_support::{rel, workspace_root};
    use sc_agent_types::{RepoRoot, Rev};

    fn minimal_value() -> serde_json::Value {
        let path = workspace_root().join("tests/fixtures/jev/subjects-schema/v1/minimal.json");
        let text = std::fs::read_to_string(path).expect("minimal.json");
        serde_json::from_str(&text).expect("json")
    }

    #[test]
    fn subjects_minimal_fixture_maps_ids_and_text() {
        let repo = RepoRoot::new(workspace_root());
        let path = rel("tests/fixtures/jev/subjects-schema/v1/minimal.json");
        let extracted = extract_subjects(&repo, &Rev::worktree(), &path).expect("subjects");
        assert_eq!(extracted.subjects[0].id(), "src/example.py:42");
        assert_eq!(extracted.subjects[1].id(), "src/example.py:42:1");
        assert_eq!(extracted.subjects[0].text(), "alpha");
        assert_eq!(extracted.subjects[1].text(), "beta");
        let document = extracted.document.expect("document");
        assert_eq!(document.schema(), SUBJECTS_SCHEMA_V1);
        assert_eq!(document.producer().mode, "matches");
        assert_eq!(extracted.subjects.len(), 2);
    }

    #[test]
    fn subjects_stdin_path_is_config_invalid() {
        let repo = RepoRoot::new(workspace_root());
        let error = extract_subjects(&repo, &Rev::worktree(), &rel("-")).expect_err("stdin");
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
