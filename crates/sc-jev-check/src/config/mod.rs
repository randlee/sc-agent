//! `check.toml` and LLM prompt paths.
//!
//! Prompt text is not sent from this module; the run facade injects it into
//! `JudgeRequest`. Production prose lives in `assets/llm-evaluator-system.md`
//! and is compiled in as [`PACKAGED_SYSTEM_PROMPT`]. That constant is the
//! only source of the packaged prompt. Packaged `assets/jev.toml` carries no
//! settings: the request byte cap belongs to each backend's descriptor
//! (REQ-SAG-0314), so this crate keeps no second copy.
//!
//! Every path in `check.toml` is a [`RepoRelativePath`], validated when the
//! document is parsed.

#![warn(missing_docs)]

use std::collections::BTreeMap;

use sc_agent_types::{BackendId, BackendSet, JevError, RepoRelativePath};
use serde::{Deserialize, Serialize};

/// Check input stored under `[inputs.<name>]` in `check.toml`.
///
/// `<name>` is the table key, not a field. Keys match
/// `docs/sc-jev-check/extraction.md`: `kind`, `path`, `column`, and for
/// `command` the argv array `command`, an optional `path` substituted for
/// `{path}`, and optional `timeout_seconds` (default 30). Only these four
/// kinds exist; a new kind is a new variant plus an ADR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum InputSpec {
    /// Read a file at the caller-supplied revision.
    File {
        /// Repo-relative path.
        path: RepoRelativePath,
    },
    /// Read one markdown table column.
    TableColumn {
        /// Repo-relative path.
        path: RepoRelativePath,
        /// Column header to extract.
        column: String,
    },
    /// Read a `sc/subjects/1` document from `path`.
    Subjects {
        /// Repo-relative path of the artifact.
        path: RepoRelativePath,
    },
    /// Run an argv array. No shell.
    Command {
        /// Argument vector. The check.toml key is `command`.
        command: Vec<String>,
        /// Process time limit in seconds. Omitted toml uses 30.
        #[serde(default = "default_command_timeout_seconds")]
        timeout_seconds: u64,
        /// Repo-relative file substituted for `{path}` (REQ-SAG-0312).
        ///
        /// Validated like a `file` input path before the command runs.
        /// `{path}` in `command` without this field is `CONFIG_INVALID`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<RepoRelativePath>,
    },
}

fn default_command_timeout_seconds() -> u64 {
    30
}

/// Packaged evaluator instructions (`include_str` of the asset file).
pub const PACKAGED_SYSTEM_PROMPT: &str = include_str!("../../assets/llm-evaluator-system.md");

/// Display name of the packaged system prompt in [`ResolvedPromptPath::path`].
pub const PACKAGED_SYSTEM_PROMPT_NAME: &str = "llm-evaluator-system.md";

/// Where a system-prompt path was selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptOrigin {
    /// `[llm_judge] system_prompt_file` in `check.toml`.
    CheckToml,
    /// The packaged asset compiled into this crate.
    PackagedAsset,
}

/// Prompt path chosen for a check. The file is not read here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPromptPath {
    /// Repo-relative path, or the packaged asset file name.
    pub path: String,
    /// Which config supplied `path`.
    pub origin: PromptOrigin,
}

/// A parsed `check.toml`.
///
/// Input names are `[inputs.<name>]` table keys. `sc-jev-check` is the only
/// crate that runs those inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckSpec {
    /// Repo-relative `plan-anchoring/questions/1` document.
    pub questions_path: RepoRelativePath,
    /// Repo-relative state document. Absent means an empty state.
    pub state_path: Option<RepoRelativePath>,
    /// Optional override of the packaged system-prompt path.
    pub system_prompt_file: Option<RepoRelativePath>,
    /// Named extractors, in table-key order.
    pub inputs: BTreeMap<String, InputSpec>,
    /// Top-level `backends` list. `RunConfig::backends` wins over it, and
    /// `[jev]` applies when both are absent (ADR-SAG-0024).
    pub backends: Option<BackendSet>,
}

impl CheckSpec {
    /// A check with only a questions document: no state, inputs, prompt
    /// override, or backends.
    #[must_use]
    pub fn new(questions_path: RepoRelativePath) -> Self {
        Self {
            questions_path,
            state_path: None,
            system_prompt_file: None,
            inputs: BTreeMap::new(),
            backends: None,
        }
    }

    /// Selects the system-prompt path without reading the file.
    ///
    /// `check.toml` wins when `system_prompt_file` is set. Otherwise the
    /// packaged asset ([`PACKAGED_SYSTEM_PROMPT_NAME`]) is used.
    #[must_use]
    pub fn resolve_prompt_path(&self) -> ResolvedPromptPath {
        match &self.system_prompt_file {
            Some(path) => ResolvedPromptPath {
                path: path.as_str().to_owned(),
                origin: PromptOrigin::CheckToml,
            },
            None => ResolvedPromptPath {
                path: PACKAGED_SYSTEM_PROMPT_NAME.to_owned(),
                origin: PromptOrigin::PackagedAsset,
            },
        }
    }
}

/// Parses `check.toml` text.
///
/// # Errors
///
/// Returns [`JevError::ConfigInvalid`] when the document does not match the
/// check shape: unknown keys, a path that is not repo-relative, an unknown
/// backend id, or an empty or repeated `backends` list.
pub fn load_check_str(text: &str) -> Result<CheckSpec, JevError> {
    let raw: RawCheck = toml::from_str(text)
        .map_err(|error| JevError::config_invalid(format!("check.toml: {error}")))?;
    for name in raw.inputs.keys() {
        if !is_input_name(name) {
            return Err(JevError::config_invalid(format!(
                "check.toml input name {name} must be [A-Za-z0-9_-]+"
            )));
        }
    }
    let backends = match raw.backends {
        Some(ids) => Some(BackendSet::try_from(ids)?),
        None => None,
    };
    Ok(CheckSpec {
        questions_path: raw.questions.path,
        state_path: raw.state.map(|state| state.path),
        system_prompt_file: raw.llm_judge.map(|llm| llm.system_prompt_file),
        inputs: raw.inputs,
        backends,
    })
}

/// Reads `check.toml` at `relative` and `rev` through the repo-root resolver.
///
/// # Errors
///
/// Returns [`JevError`] when the path cannot be read or the document is invalid.
pub fn load_check_at(
    repo: &sc_agent_types::RepoRoot,
    rev: &sc_agent_types::Rev,
    relative: &str,
) -> Result<CheckSpec, JevError> {
    let bytes = crate::extract::read_repo_bytes(repo, rev, &crate::extract::repo_path(relative)?)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| JevError::config_invalid(format!("{relative} is not UTF-8")))?;
    load_check_str(text)
}

fn is_input_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCheck {
    questions: RawPath,
    #[serde(default)]
    state: Option<RawPath>,
    #[serde(default)]
    llm_judge: Option<RawLlmJudge>,
    #[serde(default)]
    inputs: BTreeMap<String, InputSpec>,
    #[serde(default)]
    backends: Option<Vec<BackendId>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPath {
    path: RepoRelativePath,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLlmJudge {
    system_prompt_file: RepoRelativePath,
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::config::InputSpec;
    use crate::extract::test_support::rel;

    #[test]
    fn payload_packaged_system_prompt_matches_asset() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/llm-evaluator-system.md");
        let disk = std::fs::read_to_string(path).expect("asset");
        assert_eq!(disk, PACKAGED_SYSTEM_PROMPT);
        assert_eq!(
            PACKAGED_SYSTEM_PROMPT,
            include_str!("../../assets/llm-evaluator-system.md")
        );
        assert!(
            !PACKAGED_SYSTEM_PROMPT.contains("TOKEN"),
            "the shipped prompt carries no test sentinel"
        );
        assert!(PACKAGED_SYSTEM_PROMPT.contains("canonical judge payload"));
    }

    /// REQ-SAG-0314: the engine keeps no request byte cap; packaged
    /// `jev.toml` is an empty document and no code reads `max_input_bytes`.
    #[test]
    fn packaged_jev_toml_has_no_byte_cap() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/jev.toml");
        let text = std::fs::read_to_string(path).expect("jev.toml");
        let table: toml::Table = toml::from_str(&text).expect("toml");
        assert!(table.is_empty(), "{table:?}");
        assert!(!text.contains("max_input_bytes ="), "{text}");
    }

    #[test]
    fn check_paths_are_repo_relative_and_backends_are_a_set() {
        for bad in [
            "[questions]\npath = \"../q.json\"\n",
            "[questions]\npath = \"/etc/q.json\"\n",
            "[questions]\npath = \"\"\n",
            "[questions]\npath = \"q.json\"\n[llm_judge]\nsystem_prompt_file = \"\"\n",
            "[questions]\npath = \"q.json\"\n[inputs.a]\nkind = \"file\"\npath = \"../a\"\n",
            "backends = []\n[questions]\npath = \"q.json\"\n",
            "backends = [\"jev\", \"jev\"]\n[questions]\npath = \"q.json\"\n",
            "backends = [\"nope\"]\n[questions]\npath = \"q.json\"\n",
        ] {
            let error = load_check_str(bad).expect_err(bad);
            assert_eq!(error.code(), "CONFIG_INVALID", "{bad}");
        }
        let spec = load_check_str(
            "backends = [\"anthropic_haiku\", \"jev\"]\n[questions]\npath = \"q.json\"\n",
        )
        .expect("backends");
        assert_eq!(
            spec.backends.expect("set").as_slice(),
            [BackendId::AnthropicHaiku, BackendId::Jev]
        );
    }

    #[test]
    fn command_extractor_toml_omitted_timeout_is_30() {
        let spec = load_check_str(
            r#"
            [questions]
            path = "questions.json"

            [inputs.live]
            kind = "command"
            command = ["python3", "-c", "pass"]

            [inputs.with_path]
            kind = "command"
            command = ["sc-treesitter", "{path}"]
            path = "src/hook.py"
            "#,
        )
        .expect("check");
        match &spec.inputs["live"] {
            InputSpec::Command {
                timeout_seconds,
                path,
                ..
            } => {
                assert_eq!(*timeout_seconds, 30);
                assert!(path.is_none());
            }
            other => panic!("expected command, got {other:?}"),
        }
        match &spec.inputs["with_path"] {
            InputSpec::Command { path, .. } => {
                assert_eq!(
                    path.as_ref().map(RepoRelativePath::as_str),
                    Some("src/hook.py")
                );
            }
            other => panic!("expected command, got {other:?}"),
        }
    }

    #[test]
    fn check_prompt_path_overrides_the_packaged_asset() {
        let spec = load_check_str(
            r#"
            [questions]
            path = "questions.json"

            [llm_judge]
            system_prompt_file = "prompt.md"
            "#,
        )
        .expect("check");
        let resolved = spec.resolve_prompt_path();
        assert_eq!(resolved.path, "prompt.md");
        assert_eq!(resolved.origin, PromptOrigin::CheckToml);

        let bare = load_check_str(
            r#"
            [questions]
            path = "questions.json"
            "#,
        )
        .expect("bare");
        let resolved = bare.resolve_prompt_path();
        assert_eq!(resolved.origin, PromptOrigin::PackagedAsset);
        assert_eq!(resolved.path, "llm-evaluator-system.md");
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
                path: rel("skills/atm-bd-lite/scripts/atm-bd-task-hook.py"),
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
                path: rel("docs/decisions.md"),
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
                path: None,
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
