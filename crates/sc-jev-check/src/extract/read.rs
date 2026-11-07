//! Repo-relative reads at `WORKTREE` or a git revision.
//!
//! Working-tree reads resolve symlinks and refuse any target outside the
//! canonical repo root. Revision reads go through `git cat-file` with a
//! `./`-prefixed path, so a [`RepoRoot`] that is a subdirectory of the git
//! toplevel reads the same file at every revision. Both paths stop at
//! [`MAX_INPUT_BYTES`].

use std::fs::File;
use std::io::Read;
use std::path::{Component, Path};
use std::process::{Command, Output};

use sc_agent_types::{JevError, RepoRoot, Rev};

use super::sanitize::{redact_error, stderr_tail};

/// Largest single file an input may read.
///
/// This bounds memory before the canonical payload cap applies. It matches
/// the command extractor's stdout cap. The payload byte cap in packaged
/// `jev.toml` still refuses an oversize request after extraction.
pub(crate) const MAX_INPUT_BYTES: u64 = 8 * 1024 * 1024;

/// Reads `relative` from `repo` at `rev`.
///
/// `Rev::WORKTREE` reads the working tree. Any other revision reads the blob
/// at `<rev>:./<relative>` with `repo` as the git working directory.
///
/// # Errors
///
/// Returns [`JevError::InputInvalid`] for absolute paths, `..`, a symlink
/// that leaves the repo root, a non-file target, or an invalid revision.
/// Returns [`JevError::InputMissing`] when the path is absent and
/// [`JevError::InputTooLarge`] above 8 MiB.
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
    let root = std::fs::canonicalize(repo.as_path()).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            JevError::InputMissing {
                path: repo.as_path().display().to_string(),
            }
        } else {
            JevError::io_failed("repo root", &error)
        }
    })?;
    let full = match std::fs::canonicalize(repo.as_path().join(relative)) {
        Ok(full) => full,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(JevError::InputMissing {
                path: relative.to_owned(),
            });
        }
        Err(error) => return Err(JevError::io_failed(relative, &error)),
    };
    if !full.starts_with(&root) {
        return Err(JevError::InputInvalid {
            detail: format!("input path {relative} resolves outside the repo root"),
        });
    }
    let metadata =
        std::fs::metadata(&full).map_err(|error| JevError::io_failed(relative, &error))?;
    if !metadata.is_file() {
        return Err(JevError::InputInvalid {
            detail: format!("input path {relative} is not a regular file"),
        });
    }
    let file = File::open(&full).map_err(|error| JevError::io_failed(relative, &error))?;
    let mut bytes = Vec::new();
    file.take(MAX_INPUT_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| JevError::io_failed(relative, &error))?;
    let actual_bytes = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if actual_bytes > MAX_INPUT_BYTES {
        return Err(JevError::InputTooLarge {
            limit_bytes: MAX_INPUT_BYTES,
            actual_bytes: actual_bytes.max(metadata.len()),
        });
    }
    Ok(bytes)
}

fn read_git_show(repo: &RepoRoot, rev: &Rev, relative: &str) -> Result<Vec<u8>, JevError> {
    let spec = format!("{}:./{relative}", rev.as_str());
    let kind = git_cat_file(repo, &["-t", "--end-of-options", &spec])?;
    if !kind.status.success() {
        return Err(classify_git_failure(rev, relative, &kind));
    }
    if String::from_utf8_lossy(&kind.stdout).trim() != "blob" {
        return Err(JevError::InputInvalid {
            detail: format!("input path {relative} is not a file at {rev}"),
        });
    }
    let size = git_cat_file(repo, &["-s", "--end-of-options", &spec])?;
    if !size.status.success() {
        return Err(classify_git_failure(rev, relative, &size));
    }
    let actual_bytes = String::from_utf8_lossy(&size.stdout)
        .trim()
        .parse::<u64>()
        .map_err(|_| {
            JevError::operational_io(&format!(
                "git cat-file -s returned an unreadable size for {relative} at {rev}. Retry; if it persists, check the git installation."
            ))
        })?;
    if actual_bytes > MAX_INPUT_BYTES {
        return Err(JevError::InputTooLarge {
            limit_bytes: MAX_INPUT_BYTES,
            actual_bytes,
        });
    }
    let blob = git_cat_file(repo, &["blob", &spec])?;
    if !blob.status.success() {
        return Err(classify_git_failure(rev, relative, &blob));
    }
    let read_bytes = u64::try_from(blob.stdout.len()).unwrap_or(u64::MAX);
    if read_bytes > MAX_INPUT_BYTES {
        return Err(JevError::InputTooLarge {
            limit_bytes: MAX_INPUT_BYTES,
            actual_bytes: read_bytes,
        });
    }
    Ok(blob.stdout)
}

fn git_cat_file(repo: &RepoRoot, args: &[&str]) -> Result<Output, JevError> {
    Command::new("git")
        .arg("-C")
        .arg(repo.as_path())
        .arg("cat-file")
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|error| {
            JevError::operational_io(&format!(
                "git could not be started (cause: {}). Install git or put it on PATH, then retry.",
                error.kind()
            ))
        })
}

/// Maps a failed `git cat-file` to a typed error.
///
/// A path that is absent at `rev` is [`JevError::InputMissing`]. A revision
/// git cannot resolve is [`JevError::InputInvalid`] with git's trimmed
/// message. Anything else is an operational failure with a sanitized stderr
/// tail.
fn classify_git_failure(rev: &Rev, relative: &str, output: &Output) -> JevError {
    redact_error(classify_git_message(rev, relative, output))
}

fn classify_git_message(rev: &Rev, relative: &str, output: &Output) -> JevError {
    let message = stderr_tail(&output.stderr);
    let lower = message.to_ascii_lowercase();
    if lower.contains("does not exist in") || lower.contains("exists on disk, but not in") {
        return JevError::InputMissing {
            path: relative.to_owned(),
        };
    }
    if lower.contains("invalid object name")
        || lower.contains("not a valid object name")
        || lower.contains("bad revision")
        || lower.contains("unknown revision")
        || lower.contains("ambiguous argument")
    {
        return JevError::InputInvalid {
            detail: format!("revision {rev} could not be resolved: {message}"),
        };
    }
    JevError::operational_io(&format!(
        "git cat-file failed for {relative} at {rev} ({message}). Check that the repo root is a git repository and retry."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::test_support::{commit_all, git_fixture};

    #[test]
    fn subdirectory_repo_root_reads_the_same_file_at_rev_and_worktree() {
        let (dir, _sha) = git_fixture();
        std::fs::write(dir.path().join("readme.txt"), "toplevel decoy\n").expect("decoy");
        commit_all(dir.path(), "decoy");
        let sub = RepoRoot::new(dir.path().join("notes"));
        let head = read_repo_text(&sub, &Rev::new("HEAD"), "readme.txt").expect("head");
        let worktree = read_repo_text(&sub, &Rev::worktree(), "readme.txt").expect("worktree");
        assert_eq!(head, worktree);
        assert_eq!(head, "alpha note\n");
    }

    #[test]
    fn bad_revision_is_input_invalid_with_git_message() {
        let (dir, _sha) = git_fixture();
        let repo = RepoRoot::new(dir.path());
        let error = read_repo_bytes(&repo, &Rev::new("no-such-rev"), "notes/readme.txt")
            .expect_err("bad rev");
        assert_eq!(error.code(), "INPUT_INVALID");
        assert!(error.to_string().contains("no-such-rev"), "{error}");
    }

    #[test]
    fn path_absent_at_rev_is_input_missing() {
        let (dir, _sha) = git_fixture();
        let repo = RepoRoot::new(dir.path());
        let error =
            read_repo_bytes(&repo, &Rev::new("HEAD"), "notes/absent.txt").expect_err("absent");
        assert_eq!(error.code(), "INPUT_MISSING");
    }

    #[test]
    fn tree_at_rev_is_not_a_file() {
        let (dir, _sha) = git_fixture();
        let repo = RepoRoot::new(dir.path());
        let error = read_repo_bytes(&repo, &Rev::new("HEAD"), "notes").expect_err("tree");
        assert_eq!(error.code(), "INPUT_INVALID");
        assert!(error.to_string().contains("not a file"), "{error}");
    }

    #[test]
    fn worktree_directory_is_not_a_file() {
        let (dir, _sha) = git_fixture();
        let repo = RepoRoot::new(dir.path());
        let error = read_repo_bytes(&repo, &Rev::worktree(), "notes").expect_err("dir");
        assert_eq!(error.code(), "INPUT_INVALID");
    }

    #[test]
    fn oversize_worktree_file_is_input_too_large() {
        let dir = tempfile::tempdir().expect("temp");
        let big = vec![b'a'; usize::try_from(MAX_INPUT_BYTES).expect("cap") + 1];
        std::fs::write(dir.path().join("big.txt"), big).expect("write");
        let error = read_repo_bytes(&RepoRoot::new(dir.path()), &Rev::worktree(), "big.txt")
            .expect_err("too large");
        assert_eq!(error.code(), "INPUT_TOO_LARGE");
    }

    #[test]
    fn oversize_blob_at_rev_is_input_too_large() {
        let (dir, _sha) = git_fixture();
        let big = vec![b'a'; usize::try_from(MAX_INPUT_BYTES).expect("cap") + 1];
        std::fs::write(dir.path().join("big.txt"), big).expect("write");
        commit_all(dir.path(), "big");
        let error = read_repo_bytes(&RepoRoot::new(dir.path()), &Rev::new("HEAD"), "big.txt")
            .expect_err("too large");
        assert_eq!(error.code(), "INPUT_TOO_LARGE");
    }

    #[cfg(unix)]
    #[test]
    fn committed_symlink_outside_the_repo_is_input_invalid() {
        let (dir, _sha) = git_fixture();
        let outside = tempfile::tempdir().expect("outside");
        let secret = outside.path().join("secret.txt");
        std::fs::write(&secret, "OUTSIDE-SECRET").expect("secret");
        std::os::unix::fs::symlink(&secret, dir.path().join("leak.txt")).expect("symlink");
        commit_all(dir.path(), "symlink");
        let error = read_repo_bytes(&RepoRoot::new(dir.path()), &Rev::worktree(), "leak.txt")
            .expect_err("symlink");
        assert_eq!(error.code(), "INPUT_INVALID");
        assert!(!error.to_string().contains("OUTSIDE-SECRET"));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_inside_the_repo_is_read() {
        let (dir, _sha) = git_fixture();
        std::os::unix::fs::symlink("notes/readme.txt", dir.path().join("alias.txt"))
            .expect("symlink");
        let text = read_repo_text(&RepoRoot::new(dir.path()), &Rev::worktree(), "alias.txt")
            .expect("inside");
        assert_eq!(text, "alpha note\n");
    }
}
