//! Canonical `PayloadBundle` assembly.
//!
//! The builder validates question-set and state JSON, runs extractors, and
//! refuses an over-cap payload with `INPUT_TOO_LARGE`. It does not call a
//! `JudgeBackend` and does not send.

#![warn(missing_docs)]

use std::collections::BTreeMap;

mod questions;

use sc_agent_types::{
    CanonicalJudgePayload, ChoiceAnswer, JevError, PayloadBundle, Question, RepoRoot, Rev,
    payload_bundle_from_value,
};

use crate::config::{CheckSpec, PackagedConfig, ResolvedPromptPath, load_packaged_config};
use crate::extract::{Extracted, extract_input, read_repo_text};

#[doc(inline)]
pub use questions::{QUESTION_SET_SCHEMA, STATE_SCHEMA, parse_question_set, parse_state};

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
    /// System-prompt path selected for a later inject step.
    pub prompt: ResolvedPromptPath,
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
            return Err(JevError::ConfigInvalid {
                detail: "payload byte cap must be greater than zero".into(),
            });
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
        let packaged = load_packaged_config()?;
        self.build_with_packaged(spec, repo, rev, &packaged)
    }

    fn build_with_packaged(
        &self,
        spec: &CheckSpec,
        repo: &RepoRoot,
        rev: &Rev,
        packaged: &PackagedConfig,
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
        refuse_over_cap(&bundle.bytes, self.max_input_bytes)?;
        Ok(BuiltPayload {
            bundle,
            prompt: spec.resolve_prompt_path(packaged),
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
pub fn refuse_over_cap(bytes: &[u8], limit_bytes: u64) -> Result<(), JevError> {
    let actual_bytes = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if actual_bytes > limit_bytes {
        Err(JevError::InputTooLarge {
            limit_bytes,
            actual_bytes,
        })
    } else {
        Ok(())
    }
}

fn substitute_inputs(
    mut questions: Vec<Question>,
    extractions: &BTreeMap<String, Extracted>,
) -> Vec<Question> {
    let mut names: Vec<&str> = extractions.keys().map(String::as_str).collect();
    names.sort_by_key(|name| std::cmp::Reverse(name.len()));
    for question in &mut questions {
        for name in &names {
            let Some(extracted) = extractions.get(*name) else {
                continue;
            };
            let needle = format!("{{{name}}}");
            if !question.prompt.contains(&needle) {
                continue;
            }
            let replacement = extracted
                .subjects
                .iter()
                .map(|subject| subject.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            question.prompt = question.prompt.replace(&needle, &replacement);
        }
    }
    questions
}

fn encode_payload(
    questions: &[Question],
    state: &[ChoiceAnswer],
) -> Result<PayloadBundle, JevError> {
    let value = serde_json::to_value(CanonicalJudgePayload {
        questions: questions.to_vec(),
        state: state.to_vec(),
    })
    .map_err(|error| JevError::InputInvalid {
        detail: format!("payload encoding failed: {error}"),
    })?;
    payload_bundle_from_value(&value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::load_check_at;
    use crate::extract::test_support::{self, copy_fixture, git_fixture, workspace_root};
    use sc_agent_types::Rev;

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
        let text = std::str::from_utf8(&built.bundle.bytes).expect("utf-8");
        assert!(text.contains("alpha note"));
        assert!(text.contains("\"choice\":\"yes\"") || text.contains("\"choice\": \"yes\""));
        assert!(
            built.extractions["choice"]
                .subjects
                .iter()
                .any(|subject| subject.text == "ship | hold")
        );
        assert!(!text.contains("PROMPT-TOKEN-a3-check"));
        assert!(!text.contains("PACKAGED-PROMPT-TOKEN-a3"));
        assert_eq!(built.prompt.path, "prompt.md");
        assert_eq!(
            built.bundle.sha256,
            sc_agent_types::Sha256Digest::of(&built.bundle.bytes)
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
        assert_eq!(first.bundle.sha256, second.bundle.sha256);
        assert_ne!(first.prompt.path, second.prompt.path);
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
                sc_agent_types::InputSpec::File {
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
        let worktree_text = std::str::from_utf8(&worktree.bundle.bytes).expect("utf-8");
        let head_text = std::str::from_utf8(&head.bundle.bytes).expect("utf-8");
        assert!(worktree_text.contains("worktree edit"));
        assert!(head_text.contains("alpha note"));
        assert!(!head_text.contains("worktree edit"));
        assert_eq!(head.bundle.sha256, head_again.bundle.sha256);
        assert_ne!(head.bundle.sha256, worktree.bundle.sha256);
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
        let value: serde_json::Value = serde_json::from_slice(&built.bundle.bytes).expect("json");
        let questions = value["questions"].as_array().expect("questions");
        assert!(questions.len() >= 4);
        assert_eq!(built.prompt.path, "llm-evaluator-system.md");
    }
}
