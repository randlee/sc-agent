//! LLM system-prompt resolution (ADR-SAG-0019, REQ-SAG-0503).
//!
//! Precedence is the run override, then `check.toml`, then the packaged asset
//! compiled in with `include_str`. The text is not part of `prompt_sha256`.

use std::io::Read;
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

/// Largest system-prompt override file this crate reads.
///
/// Prompts are instructions, not inputs. 1 MiB is far above any useful
/// prompt and keeps a mistaken path (a log, `/dev/zero`) from exhausting memory.
pub(crate) const MAX_PROMPT_BYTES: u64 = 1024 * 1024;

fn read_override(repo: &Path, path: &Path) -> Result<String, JevError> {
    let full = if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo.join(path)
    };
    let display = full.display().to_string();
    let metadata = match std::fs::metadata(&full) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(JevError::PromptFileMissing { path: display });
        }
        Err(error) => {
            return Err(JevError::ConfigInvalid {
                detail: format!(
                    "system prompt {display} could not be read ({})",
                    error.kind()
                ),
            });
        }
    };
    if !metadata.is_file() {
        return Err(JevError::ConfigInvalid {
            detail: format!("system prompt {display} is not a regular file"),
        });
    }
    let file = std::fs::File::open(&full).map_err(|error| JevError::ConfigInvalid {
        detail: format!(
            "system prompt {display} could not be opened ({})",
            error.kind()
        ),
    })?;
    let mut bytes = Vec::new();
    file.take(MAX_PROMPT_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| JevError::ConfigInvalid {
            detail: format!(
                "system prompt {display} could not be read ({})",
                error.kind()
            ),
        })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_PROMPT_BYTES {
        return Err(JevError::ConfigInvalid {
            detail: format!("system prompt {display} exceeds {MAX_PROMPT_BYTES} bytes"),
        });
    }
    let text = String::from_utf8(bytes).map_err(|_| JevError::ConfigInvalid {
        detail: format!("system prompt {display} is not UTF-8"),
    })?;
    non_empty(&text, &display).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversize_override_is_config_invalid() {
        let dir = tempfile::tempdir().expect("temp");
        let path = dir.path().join("huge.md");
        let size = usize::try_from(MAX_PROMPT_BYTES).expect("cap") + 1;
        std::fs::write(&path, vec![b'p'; size]).expect("write");
        let error = read_override(dir.path(), &path).expect_err("too large");
        assert_eq!(error.code(), "CONFIG_INVALID");
        assert!(error.to_string().contains("exceeds"), "{error}");
    }

    #[test]
    fn directory_override_is_config_invalid() {
        let dir = tempfile::tempdir().expect("temp");
        let error = read_override(dir.path(), dir.path()).expect_err("dir");
        assert_eq!(error.code(), "CONFIG_INVALID");
    }

    #[test]
    fn missing_override_is_prompt_file_missing() {
        let dir = tempfile::tempdir().expect("temp");
        let error = read_override(dir.path(), Path::new("absent.md")).expect_err("missing");
        assert_eq!(error.code(), "PROMPT_FILE_MISSING");
    }
}
