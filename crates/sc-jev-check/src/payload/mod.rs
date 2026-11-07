//! Canonical `PayloadBundle` assembly.
//!
//! [`build_payload`] validates question-set and state JSON, runs the
//! extractions, and encodes one canonical payload. It applies no request
//! byte cap: each backend refuses an oversize request in
//! `validate_for_send` before any send (REQ-SAG-0314, ADR-SAG-0004). It
//! does not call a `JudgeBackend` and does not send.

#![warn(missing_docs)]

use std::collections::BTreeMap;

pub(crate) mod caller;
mod questions;

#[doc(inline)]
pub use caller::{PreparedCallerRequest, prepare_caller_request};

use sc_agent_types::{
    CanonicalJudgePayload, ChoiceAnswer, JevError, PayloadBundle, Question, RepoRoot, Rev,
};

use crate::config::CheckSpec;
use crate::extract::{Extracted, extract_input, read_repo_text};

pub(crate) use questions::parse_state;
#[doc(inline)]
pub use questions::{QUESTION_SET_SCHEMA, parse_question_set};

/// One check assembled into canonical bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltPayload {
    /// Sorted compact JSON and its SHA-256. Prompt text is not included.
    pub bundle: PayloadBundle,
    /// Extractor output keyed by `[inputs.<name>]`.
    pub extractions: BTreeMap<String, Extracted>,
}

/// Validates the check's documents, runs its inputs, and builds one payload.
///
/// Question-set and state schema checks run before any extraction. The
/// payload is never split. This function has no backend parameter, so a
/// refusal cannot send.
///
/// # Errors
///
/// Returns [`JevError`] for config, input, or extraction failures.
pub fn build_payload(
    spec: &CheckSpec,
    repo: &RepoRoot,
    rev: &Rev,
) -> Result<BuiltPayload, JevError> {
    let questions = parse_question_set(&read_repo_text(repo, rev, &spec.questions_path)?)?;
    let state = match &spec.state_path {
        Some(path) => parse_state(&read_repo_text(repo, rev, path)?, &questions)?,
        None => Vec::new(),
    };
    let mut extractions = BTreeMap::new();
    for (name, input) in &spec.inputs {
        extractions.insert(name.clone(), extract_input(repo, rev, input)?);
    }
    let questions = substitute_inputs(questions, &extractions);
    let bundle = encode_payload(&questions, &state)?;
    Ok(BuiltPayload {
        bundle,
        extractions,
    })
}

fn substitute_inputs(
    questions: Vec<Question>,
    extractions: &BTreeMap<String, Extracted>,
) -> Vec<Question> {
    let replacements: BTreeMap<&str, String> = extractions
        .iter()
        .map(|(name, extracted)| {
            let joined = extracted
                .subjects
                .iter()
                .map(|subject| subject.text())
                .collect::<Vec<_>>()
                .join("\n");
            (name.as_str(), joined)
        })
        .collect();
    questions
        .into_iter()
        .map(|question| {
            let prompt = expand_placeholders(question.prompt(), &replacements);
            question.with_prompt(prompt)
        })
        .collect()
}

/// Replaces each `{name}` token in one left-to-right pass.
///
/// Replacement text is emitted verbatim and never scanned again, so an
/// extracted subject that itself contains `{other}` stays literal. Braces
/// that do not form a known `{name}` are kept. The command extractor's argv
/// substitution uses the same function.
pub(crate) fn expand_placeholders(text: &str, replacements: &BTreeMap<&str, String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        if let Some(close) = after.find('}')
            && let Some(replacement) = replacements.get(&after[..close])
        {
            out.push_str(replacement);
            rest = &after[close + 1..];
            continue;
        }
        out.push('{');
        rest = after;
    }
    out.push_str(rest);
    out
}

pub(crate) fn encode_payload(
    questions: &[Question],
    state: &[ChoiceAnswer],
) -> Result<PayloadBundle, JevError> {
    PayloadBundle::encode(&CanonicalJudgePayload::new(
        questions.to_vec(),
        state.to_vec(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::load_check_at;
    use crate::extract::test_support::{self, copy_fixture, git_fixture, rel, workspace_root};
    use sc_agent_types::Rev;

    #[test]
    fn placeholder_text_inside_a_subject_is_not_re_expanded() {
        let dir = tempfile::tempdir().expect("temp");
        std::fs::write(
            dir.path().join("questions.json"),
            r#"{"schema":"plan-anchoring/questions/1","questions":{"q":{"type":"choice","instructions":"A={a} B={b} {unknown}","criteria":{"no":"No.","yes":"Yes."}}}}"#,
        )
        .expect("questions");
        std::fs::write(dir.path().join("a.txt"), "f\"{b}\" literal").expect("a");
        std::fs::write(dir.path().join("b.txt"), "BEE").expect("b");
        let mut spec = CheckSpec::new(rel("questions.json"));
        spec.inputs = BTreeMap::from([
            (
                "a".into(),
                crate::config::InputSpec::File { path: rel("a.txt") },
            ),
            (
                "b".into(),
                crate::config::InputSpec::File { path: rel("b.txt") },
            ),
        ]);
        let built =
            build_payload(&spec, &RepoRoot::new(dir.path()), &Rev::worktree()).expect("payload");
        let value: serde_json::Value = serde_json::from_slice(built.bundle.bytes()).expect("json");
        assert_eq!(
            value["questions"][0]["prompt"],
            "A=f\"{b}\" literal B=BEE {unknown}"
        );
    }

    #[test]
    fn payload_merges_questions_state_and_file_text() {
        let dir = copy_fixture();
        let repo = RepoRoot::new(dir.path());
        let rev = Rev::worktree();
        let spec = load_check_at(&repo, &rev, "check.toml").expect("check");
        let built = build_payload(&spec, &repo, &rev).expect("payload");
        let text = std::str::from_utf8(built.bundle.bytes()).expect("utf-8");
        assert!(text.contains("alpha note"));
        assert!(text.contains("\"choice\":\"yes\"") || text.contains("\"choice\": \"yes\""));
        assert!(
            built.extractions["choice"]
                .subjects
                .iter()
                .any(|subject| subject.text() == "ship | hold")
        );
        assert!(!text.contains("PROMPT-TOKEN-a3-check"));
        assert!(!text.contains(crate::config::PACKAGED_SYSTEM_PROMPT.trim()));
        assert_eq!(
            built.bundle.sha256(),
            &sc_agent_types::Sha256Digest::of(built.bundle.bytes())
        );
    }

    #[test]
    fn payload_prompt_path_does_not_change_sha256() {
        let dir = copy_fixture();
        let repo = RepoRoot::new(dir.path());
        let rev = Rev::worktree();
        let mut spec = load_check_at(&repo, &rev, "check.toml").expect("check");
        let first = build_payload(&spec, &repo, &rev).expect("first");
        spec.system_prompt_file = Some(rel("other-prompt.md"));
        let second = build_payload(&spec, &repo, &rev).expect("second");
        assert_eq!(first.bundle.sha256(), second.bundle.sha256());
        assert_ne!(
            spec.resolve_prompt_path().path,
            load_check_at(&repo, &rev, "check.toml")
                .expect("check")
                .resolve_prompt_path()
                .path
        );
    }

    #[test]
    fn payload_question_schema_is_checked_before_extraction() {
        let dir = test_support::temp_dir("bad-schema");
        std::fs::write(
            dir.path().join("questions.json"),
            r#"{"schema":"other","questions":{}}"#,
        )
        .expect("write");
        let mut spec = CheckSpec::new(rel("questions.json"));
        spec.inputs = BTreeMap::from([(
            "missing".into(),
            crate::config::InputSpec::File {
                path: rel("no-such-file.txt"),
            },
        )]);
        let error =
            build_payload(&spec, &RepoRoot::new(dir.path()), &Rev::worktree()).expect_err("schema");
        assert_eq!(error.code(), "INPUT_INVALID");
        assert!(error.to_string().contains("question set schema"));
        assert!(!error.to_string().contains("no-such-file"));
    }

    #[test]
    fn payload_git_rev_ignores_worktree_edits() {
        let (dir, sha) = git_fixture();
        let repo = RepoRoot::new(dir.path());
        let spec = load_check_at(&repo, &Rev::worktree(), "check.toml").expect("check");
        let head = build_payload(&spec, &repo, &Rev::new(&sha)).expect("head");
        std::fs::write(dir.path().join("notes/readme.txt"), "worktree edit\n").expect("edit");
        let worktree = build_payload(&spec, &repo, &Rev::worktree()).expect("worktree");
        let head_again = build_payload(&spec, &repo, &Rev::new(sha)).expect("head again");
        let worktree_text = std::str::from_utf8(worktree.bundle.bytes()).expect("utf-8");
        let head_text = std::str::from_utf8(head.bundle.bytes()).expect("utf-8");
        assert!(worktree_text.contains("worktree edit"));
        assert!(head_text.contains("alpha note"));
        assert!(!head_text.contains("worktree edit"));
        assert_eq!(head.bundle.sha256(), head_again.bundle.sha256());
        assert_ne!(head.bundle.sha256(), worktree.bundle.sha256());
    }

    #[test]
    fn payload_accepts_plan_anchoring_dev_questions() {
        let repo = RepoRoot::new(workspace_root());
        let spec = CheckSpec::new(rel(
            ".claude/skills/plan-anchoring/assets/questions/dev.json",
        ));
        let built = build_payload(&spec, &repo, &Rev::worktree()).expect("dev questions");
        let value: serde_json::Value = serde_json::from_slice(built.bundle.bytes()).expect("json");
        let questions = value["questions"].as_array().expect("questions");
        assert!(questions.len() >= 4);
        assert_eq!(spec.resolve_prompt_path().path, "llm-evaluator-system.md");
    }
}
