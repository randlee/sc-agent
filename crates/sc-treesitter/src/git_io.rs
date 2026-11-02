//! Git blob reads for the `sc-treesitter` binary.
//!
//! The library never calls git. This module is part of the binary only.

use std::path::{Path, PathBuf};
use std::process::Command;

use sc_agent_types::{JevError, Rev};

pub(crate) struct Blob {
    pub(crate) path: String,
    pub(crate) bytes: Vec<u8>,
    pub(crate) sha: String,
}

pub(crate) fn read_blob(rev: &str, path_arg: &str) -> Result<Blob, JevError> {
    let repo = repo_root()?;
    let path = repo_relative(&repo, path_arg)?;
    if rev == Rev::WORKTREE {
        let full = repo.join(&path);
        let bytes =
            std::fs::read(&full).map_err(|_| JevError::InputMissing { path: path.clone() })?;
        let sha = hash_object(&full)?;
        return Ok(Blob { path, bytes, sha });
    }

    let spec = format!("{rev}:{path}");
    let show = git(&repo, &["show", "--no-textconv", spec.as_str()])?;
    if !show.status.success() {
        return Err(git_failure(rev, &path, &show.stderr));
    }
    let parsed = git(&repo, &["rev-parse", "--verify", spec.as_str()])?;
    if !parsed.status.success() {
        return Err(git_failure(rev, &path, &parsed.stderr));
    }
    let sha = String::from_utf8(parsed.stdout).map_err(|_| JevError::ExtractionFailed {
        detail: "git rev-parse returned a non-utf8 blob id".into(),
    })?;
    let sha = sha.trim().to_owned();
    if sha.len() != 40
        || !sha
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(JevError::ExtractionFailed {
            detail: "git rev-parse returned an unexpected blob id".into(),
        });
    }
    Ok(Blob {
        path,
        bytes: show.stdout,
        sha,
    })
}

fn repo_relative(repo: &Path, path_arg: &str) -> Result<String, JevError> {
    if path_arg.is_empty() {
        return Err(JevError::InputMissing {
            path: path_arg.to_owned(),
        });
    }
    let path = Path::new(path_arg);
    let relative = if path.is_absolute() {
        path.strip_prefix(repo)
            .map(Path::to_path_buf)
            .map_err(|_| JevError::InputInvalid {
                detail: format!("{path_arg} is outside the repository"),
            })?
    } else {
        PathBuf::from(path_arg.replace('\\', "/"))
    };
    let text = relative.to_string_lossy().replace('\\', "/");
    let text = text.trim_start_matches("./").to_owned();
    if text.is_empty()
        || text.split('/').any(|part| part.is_empty() || part == "..")
        || text.starts_with('/')
    {
        return Err(JevError::InputInvalid {
            detail: format!("{path_arg} is not a repo-relative path"),
        });
    }
    Ok(text)
}

fn repo_root() -> Result<PathBuf, JevError> {
    let output = Command::new("git")
        .args(["--no-pager", "rev-parse", "--show-toplevel"])
        .output()
        .map_err(|error| JevError::ExtractionFailed {
            detail: format!("git failed to start: {error}"),
        })?;
    if !output.status.success() {
        return Err(JevError::ExtractionFailed {
            detail: "git rev-parse --show-toplevel failed; run sc-treesitter inside a repository"
                .into(),
        });
    }
    let text = String::from_utf8(output.stdout).map_err(|_| JevError::ExtractionFailed {
        detail: "git rev-parse returned a non-utf8 path".into(),
    })?;
    Ok(PathBuf::from(text.trim()))
}

fn hash_object(path: &Path) -> Result<String, JevError> {
    let output = Command::new("git")
        .args(["--no-pager", "hash-object", "--"])
        .arg(path)
        .output()
        .map_err(|error| JevError::ExtractionFailed {
            detail: format!("git failed to start: {error}"),
        })?;
    if !output.status.success() {
        return Err(JevError::ExtractionFailed {
            detail: format!("git hash-object failed: {}", brief(&output.stderr)),
        });
    }
    let sha = String::from_utf8(output.stdout).map_err(|_| JevError::ExtractionFailed {
        detail: "git hash-object returned a non-utf8 blob id".into(),
    })?;
    Ok(sha.trim().to_owned())
}

fn git(repo: &Path, args: &[&str]) -> Result<std::process::Output, JevError> {
    Command::new("git")
        .arg("--no-pager")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|error| JevError::ExtractionFailed {
            detail: format!("git failed to start: {error}"),
        })
}

fn git_failure(rev: &str, path: &str, stderr: &[u8]) -> JevError {
    let text = String::from_utf8_lossy(stderr);
    if text.contains("does not exist") || text.contains("exists on disk") {
        JevError::InputMissing {
            path: path.to_owned(),
        }
    } else if text.contains("invalid object name") || text.contains("bad revision") {
        JevError::InputInvalid {
            detail: format!("revision {rev} was not found"),
        }
    } else {
        JevError::ExtractionFailed {
            detail: format!("git show {rev}:{path} failed: {}", brief(stderr)),
        }
    }
}

fn brief(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let line = text.lines().next().unwrap_or("").trim();
    let mut short = line.chars().take(200).collect::<String>();
    if line.chars().count() > 200 {
        short.push('…');
    }
    if short.is_empty() {
        "git exited non-zero".to_owned()
    } else {
        short
    }
}
