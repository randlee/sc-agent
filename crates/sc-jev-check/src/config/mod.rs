//! `check.toml`, packaged `jev.toml`, and LLM prompt paths.
//!
//! Prompt text is not sent from this module; the run facade injects it into
//! `JudgeRequest`. Production prose lives in `assets/llm-evaluator-system.md`
//! and is compiled in as [`PACKAGED_SYSTEM_PROMPT`]. That constant is the
//! only source of the packaged prompt; packaged `jev.toml` carries limits
//! only and is parsed once per process.

#![warn(missing_docs)]

use std::collections::BTreeMap;
use std::sync::OnceLock;

use sc_agent_types::JevError;
use serde::{Deserialize, Serialize};

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

/// Packaged evaluator instructions (`include_str` of the asset file).
pub const PACKAGED_SYSTEM_PROMPT: &str = include_str!("../../assets/llm-evaluator-system.md");

/// Display name of the packaged system prompt in [`ResolvedPromptPath::path`].
pub const PACKAGED_SYSTEM_PROMPT_NAME: &str = "llm-evaluator-system.md";

const PACKAGED_JEV_TOML: &str = include_str!("../../assets/jev.toml");

/// Where a system-prompt path was selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptOrigin {
    /// `[llm_judge] system_prompt_file` in `check.toml`.
    CheckToml,
    /// `system_prompt_file` in packaged `jev.toml`.
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

/// Defaults loaded from packaged `assets/jev.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackagedConfig {
    /// Canonical payload byte cap (`INPUT_TOO_LARGE` above this).
    pub max_input_bytes: u64,
}

/// A parsed `check.toml`.
///
/// Input names are `[inputs.<name>]` table keys. `sc-jev-check` is the only
/// crate that runs those inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckSpec {
    /// Repo-relative `plan-anchoring/questions/1` document.
    pub questions_path: String,
    /// Repo-relative state document. Absent means an empty state.
    pub state_path: Option<String>,
    /// Optional override of the packaged system-prompt path.
    pub system_prompt_file: Option<String>,
    /// Named extractors, in table-key order.
    pub inputs: BTreeMap<String, InputSpec>,
}

impl CheckSpec {
    /// Selects the system-prompt path without reading the file.
    ///
    /// `check.toml` wins when `system_prompt_file` is set. Otherwise the
    /// packaged asset ([`PACKAGED_SYSTEM_PROMPT_NAME`]) is used.
    #[must_use]
    pub fn resolve_prompt_path(&self) -> ResolvedPromptPath {
        match &self.system_prompt_file {
            Some(path) => ResolvedPromptPath {
                path: path.clone(),
                origin: PromptOrigin::CheckToml,
            },
            None => ResolvedPromptPath {
                path: PACKAGED_SYSTEM_PROMPT_NAME.to_owned(),
                origin: PromptOrigin::PackagedAsset,
            },
        }
    }
}

/// Returns the packaged byte cap from `assets/jev.toml`.
///
/// The compiled-in file is parsed once per process and cached.
///
/// # Errors
///
/// Returns [`JevError::ConfigInvalid`] when `assets/jev.toml` is malformed.
pub fn load_packaged_config() -> Result<PackagedConfig, JevError> {
    static PACKAGED: OnceLock<Result<PackagedConfig, JevError>> = OnceLock::new();
    PACKAGED
        .get_or_init(|| parse_packaged(PACKAGED_JEV_TOML))
        .clone()
}

/// Parses `check.toml` text.
///
/// # Errors
///
/// Returns [`JevError::ConfigInvalid`] when the document does not match the
/// Phase A check shape.
pub fn load_check_str(text: &str) -> Result<CheckSpec, JevError> {
    let raw: RawCheck = toml::from_str(text)
        .map_err(|error| JevError::config_invalid(format!("check.toml: {error}")))?;
    if raw.questions.path.is_empty() {
        return Err(JevError::config_invalid(
            "check.toml questions.path is empty",
        ));
    }
    if let Some(prompt) = &raw.llm_judge
        && prompt.system_prompt_file.is_empty()
    {
        return Err(JevError::config_invalid(
            "check.toml llm_judge.system_prompt_file is empty",
        ));
    }
    for name in raw.inputs.keys() {
        if !is_input_name(name) {
            return Err(JevError::config_invalid(format!(
                "check.toml input name {name} must be [A-Za-z0-9_-]+"
            )));
        }
    }
    Ok(CheckSpec {
        questions_path: raw.questions.path,
        state_path: raw.state.map(|state| state.path),
        system_prompt_file: raw.llm_judge.map(|llm| llm.system_prompt_file),
        inputs: raw.inputs,
    })
}

/// Reads `check.toml` through the repo-root resolver.
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

fn parse_packaged(text: &str) -> Result<PackagedConfig, JevError> {
    let raw: RawPackaged = toml::from_str(text)
        .map_err(|error| JevError::config_invalid(format!("jev.toml: {error}")))?;
    if raw.limits.max_input_bytes == 0 {
        return Err(JevError::config_invalid(
            "jev.toml limits.max_input_bytes must be greater than zero",
        ));
    }
    Ok(PackagedConfig {
        max_input_bytes: raw.limits.max_input_bytes,
    })
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
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPath {
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLlmJudge {
    system_prompt_file: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPackaged {
    limits: RawLimits,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLimits {
    max_input_bytes: u64,
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::config::InputSpec;

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

    #[test]
    fn payload_packaged_jev_toml_sets_the_byte_cap() {
        let packaged = load_packaged_config().expect("jev.toml");
        assert_eq!(packaged.max_input_bytes, 200_000);
        assert_eq!(load_packaged_config().expect("cached"), packaged);
        assert!(parse_packaged("[limits]\nmax_input_bytes = 0\n").is_err());
        assert!(
            parse_packaged("[limits]\nmax_input_bytes = 1\n[vendors.x]\nmodel = \"m\"\n").is_err(),
            "vendor pins are no longer part of jev.toml"
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
            "#,
        )
        .expect("check");
        match &spec.inputs["live"] {
            InputSpec::Command {
                timeout_seconds, ..
            } => assert_eq!(*timeout_seconds, 30),
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
