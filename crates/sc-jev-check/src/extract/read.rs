//! Repo-relative reads at `WORKTREE` or `git show`.

use std::path::{Component, Path};
use std::process::Command;

use sc_agent_types::{JevError, RepoRoot, Rev};

/// Reads `relative` from `repo` at `rev`.
///
/// `Rev::WORKTREE` reads the working tree. Any other revision uses
/// `git show <rev>:<path>` with `repo` as the git toplevel.
///
/// # Errors
///
/// Returns [`JevError::InputInvalid`] for absolute paths, `..`, or a failed
/// git invocation. Returns [`JevError::InputMissing`] when the path is absent.
pub fn read_repo_bytes(repo: &RepoRoot, rev: &Rev, relative: &str) -> Result<Vec<u8>, JevError> {
    validate_relative(relative)?;
    if rev.as_str() == Rev::WORKTREE {
        return read_worktree(repo, relative);
    }
    read_git_show(repo, rev, relative)
}

/// UTF-8 form of [`read_repo_bytes`].
///
/// # Errors
///
/// Returns [`JevError::InputInvalid`] when the bytes are not UTF-8, plus the
/// errors of [`read_repo_bytes`].
pub fn read_repo_text(repo: &RepoRoot, rev: &Rev, relative: &str) -> Result<String, JevError> {
    let bytes = read_repo_bytes(repo, rev, relative)?;
    String::from_utf8(bytes).map_err(|_| JevError::InputInvalid {
        detail: format!("{relative} is not UTF-8"),
    })
}

fn validate_relative(relative: &str) -> Result<(), JevError> {
    if relative.is_empty()
        || relative.starts_with('/')
        || relative.contains('\\')
        || relative.contains('\0')
        || relative.contains(':')
    {
        return Err(JevError::InputInvalid {
            detail: format!("input path {relative} must be a repo-relative path"),
        });
    }
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(JevError::InputInvalid {
            detail: format!("input path {relative} escapes the repo root"),
        });
    }
    Ok(())
}

fn read_worktree(repo: &RepoRoot, relative: &str) -> Result<Vec<u8>, JevError> {
    let path = repo.as_path().join(relative);
    match std::fs::read(&path) {
        Ok(bytes) => Ok(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(JevError::InputMissing {
            path: relative.to_owned(),
        }),
        Err(error) => Err(JevError::InputInvalid {
            detail: format!("failed to read {relative}: {error}"),
        }),
    }
}

fn read_git_show(repo: &RepoRoot, rev: &Rev, relative: &str) -> Result<Vec<u8>, JevError> {
    let spec = format!("{}:{relative}", rev.as_str());
    let output = Command::new("git")
        .arg("-C")
        .arg(repo.as_path())
        .arg("show")
        .arg("--end-of-options")
        .arg(&spec)
        .output()
        .map_err(|error| JevError::InputInvalid {
            detail: format!("git show failed to start: {error}"),
        })?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(JevError::InputMissing {
            path: relative.to_owned(),
        })
    }
}
