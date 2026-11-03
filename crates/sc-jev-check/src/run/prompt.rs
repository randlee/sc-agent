//! LLM system-prompt resolution (ADR-SAG-0019, REQ-SAG-0503).
//!
//! Precedence is the run override, then `check.toml`, then the packaged asset
//! compiled in with `include_str`. The text is not part of `prompt_sha256`.

use std::path::Path;

use sc_agent_types::{JevError, RepoRoot, Rev};

use crate::config::{CheckSpec, PromptOrigin, load_packaged_config};
use crate::extract::read_repo_text;

pub(crate) fn load_request_system_prompt(
    repo: &RepoRoot,
    override_path: Option<&Path>,
    needs_llm: bool,
) -> Result<Option<String>, JevError> {
    if !needs_llm {
        return Ok(None);
    }
    if let Some(path) = override_path {
        return Ok(Some(read_override(repo.as_path(), path)?));
    }
    non_empty(
        crate::config::PACKAGED_SYSTEM_PROMPT,
        "packaged system prompt",
    )
    .map(str::to_owned)
    .map(Some)
}

pub(crate) fn load_system_prompt(
    spec: &CheckSpec,
    repo: &RepoRoot,
    rev: &Rev,
    override_path: Option<&Path>,
    needs_llm: bool,
) -> Result<Option<String>, JevError> {
    if !needs_llm {
        return Ok(None);
    }
    if let Some(path) = override_path {
        return Ok(Some(read_override(repo.as_path(), path)?));
    }
    let packaged = load_packaged_config()?;
    let resolved = spec.resolve_prompt_path(&packaged);
    match resolved.origin {
        PromptOrigin::PackagedAsset => {
            non_empty(crate::config::PACKAGED_SYSTEM_PROMPT, &resolved.path)
                .map(str::to_owned)
                .map(Some)
        }
        PromptOrigin::CheckToml => {
            let text = match read_repo_text(repo, rev, &resolved.path) {
                Ok(text) => text,
                Err(JevError::InputMissing { path }) => {
                    return Err(JevError::PromptFileMissing { path });
                }
                Err(error) => return Err(error),
            };
            non_empty(&text, &resolved.path)
                .map(str::to_owned)
                .map(Some)
        }
    }
}

fn non_empty<'a>(text: &'a str, path: &str) -> Result<&'a str, JevError> {
    if text.trim().is_empty() {
        Err(JevError::ConfigInvalid {
            detail: format!("system prompt {path} is empty"),
        })
    } else {
        Ok(text)
    }
}

fn read_override(repo: &Path, path: &Path) -> Result<String, JevError> {
    let full = if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo.join(path)
    };
    let display = full.display().to_string();
    match std::fs::read_to_string(&full) {
        Ok(text) => non_empty(&text, &display).map(str::to_owned),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(JevError::PromptFileMissing { path: display })
        }
        Err(error) => Err(JevError::ConfigInvalid {
            detail: format!("system prompt {display}: {error}"),
        }),
    }
}
