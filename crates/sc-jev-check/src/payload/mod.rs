//! Canonical `PayloadBundle` assembly.
//!
//! The builder validates question-set and state JSON, runs extractors, and
//! refuses an over-cap payload with `INPUT_TOO_LARGE`. It does not call a
//! `JudgeBackend` and does not send.

#![warn(missing_docs)]

use std::collections::BTreeMap;

mod caller;
mod questions;

#[doc(inline)]
pub use caller::{PreparedCallerRequest, prepare_caller_request};

use sc_agent_types::{
    CanonicalJudgePayload, ChoiceAnswer, JevError, PayloadBundle, Question, RepoRoot, Rev,
};

use crate::config::{CheckSpec, load_packaged_config};
use crate::extract::{Extracted, extract_input, read_repo_text};

pub(crate) use questions::parse_state;
#[doc(inline)]
pub use questions::{QUESTION_SET_SCHEMA, parse_question_set};

/// Byte cap applied while building one canonical payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayloadBuilder {
    max_input_bytes: u64,
}

/// One check assembled into canonical bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltPayload {
    /// Sorted compact JSON and its SHA-256. Prompt text is not included.
    pub bundle: PayloadBundle,
    /// Extractor output keyed by `[inputs.<name>]`.
    pub extractions: BTreeMap<String, Extracted>,
}

impl PayloadBuilder {
    /// Builds payloads that refuse more than `max_input_bytes`.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::ConfigInvalid`] when the cap is zero.
    pub fn new(max_input_bytes: u64) -> Result<Self, JevError> {
        if max_input_bytes == 0 {
            return Err(JevError::config_invalid(
                "payload byte cap must be greater than zero",
            ));
        }
        Ok(Self { max_input_bytes })
    }

    /// Uses `limits.max_input_bytes` from packaged `jev.toml`.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::ConfigInvalid`] when the packaged file is invalid.
    pub fn packaged() -> Result<Self, JevError> {
        let packaged = load_packaged_config()?;
        Self::new(packaged.max_input_bytes)
    }

    /// Returns the configured byte cap.
    #[must_use]
    pub fn limit_bytes(&self) -> u64 {
        self.max_input_bytes
    }

    /// Validates inputs and builds one canonical payload.
    ///
    /// Question-set and state schema checks run before extractors. Over-cap
    /// payloads return [`JevError::InputTooLarge`] and are not split. This
    /// method has no backend parameter, so refusal cannot send.
    ///
    /// # Errors
    ///
    /// Returns [`JevError`] for config, input, extraction, or size failures.
    pub fn build(
        &self,
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
        refuse_over_cap(bundle.bytes(), self.max_input_bytes)?;
        Ok(BuiltPayload {
            bundle,
            extractions,
        })
    }
}

/// Returns [`JevError::InputTooLarge`] when `bytes` exceed `limit_bytes`.
///
/// # Errors
///
/// Returns [`JevError::InputTooLarge`] and nothing else. Callers must not send
/// after this error.
pub(crate) fn refuse_over_cap(bytes: &[u8], limit_bytes: u64) -> Result<(), JevError> {
    let actual_bytes = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if actual_bytes > limit_bytes {
        Err(JevError::input_too_large(limit_bytes, actual_bytes))
    } else {
        Ok(())
    }
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
    use crate::extract::test_support::{self, copy_fixture, git_fixture, workspace_root};
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
        let spec = CheckSpec {
            questions_path: "questions.json".into(),
            state_path: None,
            system_prompt_file: None,
            inputs: BTreeMap::from([
                (
                    "a".into(),
                    crate::config::InputSpec::File {
                        path: "a.txt".into(),
                    },
                ),
                (
                    "b".into(),
                    crate::config::InputSpec::File {
                        path: "b.txt".into(),
                    },
                ),
            ]),
        };
        let built = PayloadBuilder::packaged()
            .expect("builder")
            .build(&spec, &RepoRoot::new(dir.path()), &Rev::worktree())
            .expect("payload");
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
        let built = PayloadBuilder::packaged()
            .expect("builder")
            .build(&spec, &repo, &rev)
            .expect("payload");
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
        let builder = PayloadBuilder::packaged().expect("builder");
        let first = builder.build(&spec, &repo, &rev).expect("first");
        spec.system_prompt_file = Some("other-prompt.md".into());
        let second = builder.build(&spec, &repo, &rev).expect("second");
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
    fn payload_refuses_input_too_large_without_backend() {
        let dir = copy_fixture();
        let repo = RepoRoot::new(dir.path());
        let rev = Rev::worktree();
        let spec = load_check_at(&repo, &rev, "check.toml").expect("check");
        let error = PayloadBuilder::new(1)
            .expect("tiny cap")
            .build(&spec, &repo, &rev)
            .expect_err("over cap");
        assert_eq!(error.code(), "INPUT_TOO_LARGE");
        match error {
            JevError::InputTooLarge {
                limit_bytes,
                actual_bytes,
                ..
            } => {
                assert_eq!(limit_bytes, 1);
                assert!(actual_bytes > 1);
            }
            other => panic!("unexpected {other:?}"),
        }
        let packaged = PayloadBuilder::packaged().expect("packaged");
        assert_eq!(packaged.limit_bytes(), 200_000);
        assert!(packaged.build(&spec, &repo, &rev).is_ok());
    }

    #[test]
    fn payload_question_schema_is_checked_before_extraction() {
        let dir = test_support::temp_dir("bad-schema");
        std::fs::write(
            dir.path().join("questions.json"),
            r#"{"schema":"other","questions":{}}"#,
        )
        .expect("write");
        let spec = CheckSpec {
            questions_path: "questions.json".into(),
            state_path: None,
            system_prompt_file: None,
            inputs: BTreeMap::from([(
                "missing".into(),
                crate::config::InputSpec::File {
                    path: "no-such-file.txt".into(),
                },
            )]),
        };
        let error = PayloadBuilder::new(200_000)
            .expect("builder")
            .build(&spec, &RepoRoot::new(dir.path()), &Rev::worktree())
            .expect_err("schema");
        assert_eq!(error.code(), "INPUT_INVALID");
        assert!(error.to_string().contains("question set schema"));
        assert!(!error.to_string().contains("no-such-file"));
    }

    #[test]
    fn payload_git_rev_ignores_worktree_edits() {
        let (dir, sha) = git_fixture();
        let repo = RepoRoot::new(dir.path());
        let spec = load_check_at(&repo, &Rev::worktree(), "check.toml").expect("check");
        let builder = PayloadBuilder::packaged().expect("builder");
        let head = builder.build(&spec, &repo, &Rev::new(&sha)).expect("head");
        std::fs::write(dir.path().join("notes/readme.txt"), "worktree edit\n").expect("edit");
        let worktree = builder
            .build(&spec, &repo, &Rev::worktree())
            .expect("worktree");
        let head_again = builder
            .build(&spec, &repo, &Rev::new(sha))
            .expect("head again");
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
        let spec = CheckSpec {
            questions_path: ".claude/skills/plan-anchoring/assets/questions/dev.json".into(),
            state_path: None,
            system_prompt_file: None,
            inputs: BTreeMap::new(),
        };
        let built = PayloadBuilder::packaged()
            .expect("builder")
            .build(&spec, &repo, &Rev::worktree())
            .expect("dev questions");
        let value: serde_json::Value = serde_json::from_slice(built.bundle.bytes()).expect("json");
        let questions = value["questions"].as_array().expect("questions");
        assert!(questions.len() >= 4);
        assert_eq!(spec.resolve_prompt_path().path, "llm-evaluator-system.md");
    }
}
