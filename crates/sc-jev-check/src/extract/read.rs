//! Repo-relative reads at `WORKTREE` or a git revision.
//!
//! Working-tree reads resolve symlinks and refuse any target outside the
//! canonical repo root. Revision reads go through
//! [`sc_agent_types::git::read_at_rev`] (RBQA-F006): argv-only git, the
//! revision after `--end-of-options`, one resolved object id, and a reader
//! bounded by [`MAX_INPUT_BYTES`]. A [`RepoRoot`] that is a subdirectory of
//! the git toplevel reads the same file at every revision: the path is
//! prefixed with the subdirectory before the toplevel read.

use std::fs::File;
use std::io::Read;

use sc_agent_types::git::{discover_toplevel, read_at_rev};
use sc_agent_types::{JevError, RepoRelativePath, RepoRoot, Rev};

/// Largest single file an input may read.
///
/// A resource guard that bounds memory per read. It matches the command
/// extractor's stdout cap. It is not a request cap: each backend refuses an
/// oversize request in `validate_for_send` (REQ-SAG-0314).
pub(crate) const MAX_INPUT_BYTES: u64 = 8 * 1024 * 1024;

/// Reads `relative` from `repo` at `rev`.
///
/// `Rev::WORKTREE` reads the working tree. Any other revision reads the blob
/// at `rev` through `sc_agent_types::git::read_at_rev`.
///
/// # Errors
///
/// The path is already a [`RepoRelativePath`], so absolute paths and `..`
/// cannot reach this function. Returns [`JevError::InputInvalid`] for a
/// symlink that leaves the repo root, a non-file target, or an invalid
/// revision.
/// Returns [`JevError::InputMissing`] when the path is absent and
/// [`JevError::InputTooLarge`] above 8 MiB.
pub(crate) fn read_repo_bytes(
    repo: &RepoRoot,
    rev: &Rev,
    relative: &RepoRelativePath,
) -> Result<Vec<u8>, JevError> {
    if rev.as_str() == Rev::WORKTREE {
        return read_worktree(repo, relative.as_str());
    }
    read_git_blob(repo, rev, relative)
}

/// UTF-8 form of [`read_repo_bytes`].
///
/// # Errors
///
/// Returns [`JevError::InputInvalid`] when the bytes are not UTF-8, plus the
/// errors of [`read_repo_bytes`].
pub(crate) fn read_repo_text(
    repo: &RepoRoot,
    rev: &Rev,
    relative: &RepoRelativePath,
) -> Result<String, JevError> {
    let bytes = read_repo_bytes(repo, rev, relative)?;
    String::from_utf8(bytes).map_err(|error| {
        JevError::input_invalid(format!(
            "{relative} is not UTF-8 (first invalid byte at offset {})",
            error.utf8_error().valid_up_to()
        ))
    })
}

/// Checks that `relative` names a readable input file at `rev` without
/// keeping its bytes.
///
/// At `WORKTREE` this is the `file` input rule: the path is canonicalized,
/// must stay under the canonical repo root, and must be a regular file. At
/// any other revision the path must be a blob at `rev` (read and dropped,
/// bounded by [`MAX_INPUT_BYTES`]). A `command` input's `path` uses this
/// before `{path}` is substituted (REQ-SAG-0312).
///
/// # Errors
///
/// The errors of [`read_repo_bytes`] for the same path.
pub(crate) fn validate_input_path(
    repo: &RepoRoot,
    rev: &Rev,
    relative: &RepoRelativePath,
) -> Result<(), JevError> {
    if rev.as_str() == Rev::WORKTREE {
        resolve_worktree_file(repo, relative.as_str()).map(|_| ())
    } else {
        read_repo_bytes(repo, rev, relative).map(|_| ())
    }
}

/// Canonical path and metadata of a worktree input file inside the repo root.
fn resolve_worktree_file(
    repo: &RepoRoot,
    relative: &str,
) -> Result<(std::path::PathBuf, std::fs::Metadata), JevError> {
    let root = std::fs::canonicalize(repo.as_path()).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            JevError::input_missing(repo.as_path().display().to_string())
        } else {
            JevError::io_failed("repo root", &error)
        }
    })?;
    let full = match std::fs::canonicalize(repo.as_path().join(relative)) {
        Ok(full) => full,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(JevError::input_missing(relative.to_owned()));
        }
        Err(error) => return Err(JevError::io_failed(relative, &error)),
    };
    if !full.starts_with(&root) {
        return Err(JevError::input_invalid(format!(
            "input path {relative} resolves outside the repo root"
        )));
    }
    let metadata =
        std::fs::metadata(&full).map_err(|error| JevError::io_failed(relative, &error))?;
    if !metadata.is_file() {
        return Err(JevError::input_invalid(format!(
            "input path {relative} is not a regular file"
        )));
    }
    Ok((full, metadata))
}

fn read_worktree(repo: &RepoRoot, relative: &str) -> Result<Vec<u8>, JevError> {
    let (full, metadata) = resolve_worktree_file(repo, relative)?;
    let file = File::open(&full).map_err(|error| JevError::io_failed(relative, &error))?;
    let mut bytes = Vec::new();
    file.take(MAX_INPUT_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| JevError::io_failed(relative, &error))?;
    let actual_bytes = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if actual_bytes > MAX_INPUT_BYTES {
        return Err(JevError::input_too_large(
            MAX_INPUT_BYTES,
            actual_bytes.max(metadata.len()),
        ));
    }
    Ok(bytes)
}

/// Reads `relative` (under `repo`) at `rev` with `git::read_at_rev`.
///
/// `read_at_rev` names paths from the git toplevel, so when `repo` is a
/// subdirectory of its toplevel the subdirectory is prefixed first.
fn read_git_blob(
    repo: &RepoRoot,
    rev: &Rev,
    relative: &RepoRelativePath,
) -> Result<Vec<u8>, JevError> {
    let Some(toplevel) = discover_toplevel(repo.as_path())? else {
        // Not a work tree: read_at_rev reports the revision as not found.
        return read_at_rev(repo, rev, relative, MAX_INPUT_BYTES).map(|blob| blob.into_bytes());
    };
    let from_toplevel = toplevel_relative(repo, &toplevel, relative)?;
    read_at_rev(&toplevel, rev, &from_toplevel, MAX_INPUT_BYTES).map(|blob| blob.into_bytes())
}

/// `relative` as a path from `toplevel`, when `repo` is below it.
fn toplevel_relative(
    repo: &RepoRoot,
    toplevel: &RepoRoot,
    relative: &RepoRelativePath,
) -> Result<RepoRelativePath, JevError> {
    let canonical = |path: &std::path::Path| {
        std::fs::canonicalize(path).map_err(|error| JevError::io_failed("repo root", &error))
    };
    let repo_path = canonical(repo.as_path())?;
    let top_path = canonical(toplevel.as_path())?;
    let Ok(prefix) = repo_path.strip_prefix(&top_path) else {
        return Ok(relative.clone());
    };
    let mut parts: Vec<String> = Vec::new();
    for component in prefix.components() {
        let part = component.as_os_str().to_str().ok_or_else(|| {
            JevError::input_invalid("the repo root path below the git toplevel is not UTF-8")
        })?;
        parts.push(part.to_owned());
    }
    if parts.is_empty() {
        return Ok(relative.clone());
    }
    parts.push(relative.as_str().to_owned());
    RepoRelativePath::try_from(parts.join("/"))
}

/// Converts a `check.toml` path into the one repo-relative path type.
///
/// # Errors
///
/// `INPUT_INVALID` when `text` is not a repo-relative path.
pub(crate) fn repo_path(text: &str) -> Result<RepoRelativePath, JevError> {
    RepoRelativePath::try_from(text)
}

#[cfg(test)]
mod tests {
    fn rel(text: &str) -> sc_agent_types::RepoRelativePath {
        sc_agent_types::RepoRelativePath::try_from(text).expect("path")
    }

    use super::*;
    use crate::extract::test_support::{commit_all, git_fixture};

    #[test]
    fn subdirectory_repo_root_reads_the_same_file_at_rev_and_worktree() {
        let (dir, _sha) = git_fixture();
        std::fs::write(dir.path().join("readme.txt"), "toplevel decoy\n").expect("decoy");
        commit_all(dir.path(), "decoy");
        let sub = RepoRoot::new(dir.path().join("notes"));
        let head = read_repo_text(&sub, &Rev::new("HEAD"), &rel("readme.txt")).expect("head");
        let worktree =
            read_repo_text(&sub, &Rev::worktree(), &rel("readme.txt")).expect("worktree");
        assert_eq!(head, worktree);
        assert_eq!(head, "alpha note\n");
    }

    #[test]
    fn non_utf8_detail_names_the_offset() {
        let dir = tempfile::tempdir().expect("temp");
        std::fs::write(dir.path().join("bad.txt"), b"ok\xffno").expect("write");
        let error = read_repo_text(
            &RepoRoot::new(dir.path()),
            &Rev::worktree(),
            &rel("bad.txt"),
        )
        .expect_err("not utf-8");
        assert_eq!(error.code(), "INPUT_INVALID");
        assert!(error.to_string().contains("offset 2"), "{error}");
    }

    #[test]
    fn bad_revision_is_input_invalid_with_git_message() {
        let (dir, _sha) = git_fixture();
        let repo = RepoRoot::new(dir.path());
        let error = read_repo_bytes(&repo, &Rev::new("no-such-rev"), &rel("notes/readme.txt"))
            .expect_err("bad rev");
        assert_eq!(error.code(), "INPUT_INVALID");
        assert!(error.to_string().contains("no-such-rev"), "{error}");
    }

    #[test]
    fn path_absent_at_rev_is_input_missing() {
        let (dir, _sha) = git_fixture();
        let repo = RepoRoot::new(dir.path());
        let error = read_repo_bytes(&repo, &Rev::new("HEAD"), &rel("notes/absent.txt"))
            .expect_err("absent");
        assert_eq!(error.code(), "INPUT_MISSING");
    }

    #[test]
    fn tree_at_rev_is_not_a_file() {
        let (dir, _sha) = git_fixture();
        let repo = RepoRoot::new(dir.path());
        let error = read_repo_bytes(&repo, &Rev::new("HEAD"), &rel("notes")).expect_err("tree");
        assert_eq!(error.code(), "INPUT_INVALID");
        assert!(error.to_string().contains("not a file"), "{error}");
    }

    #[test]
    fn worktree_directory_is_not_a_file() {
        let (dir, _sha) = git_fixture();
        let repo = RepoRoot::new(dir.path());
        let error = read_repo_bytes(&repo, &Rev::worktree(), &rel("notes")).expect_err("dir");
        assert_eq!(error.code(), "INPUT_INVALID");
    }

    #[test]
    fn oversize_worktree_file_is_input_too_large() {
        let dir = tempfile::tempdir().expect("temp");
        let big = vec![b'a'; usize::try_from(MAX_INPUT_BYTES).expect("cap") + 1];
        std::fs::write(dir.path().join("big.txt"), big).expect("write");
        let error = read_repo_bytes(
            &RepoRoot::new(dir.path()),
            &Rev::worktree(),
            &rel("big.txt"),
        )
        .expect_err("too large");
        assert_eq!(error.code(), "INPUT_TOO_LARGE");
    }

    #[test]
    fn oversize_blob_at_rev_is_input_too_large() {
        let (dir, _sha) = git_fixture();
        let big = vec![b'a'; usize::try_from(MAX_INPUT_BYTES).expect("cap") + 1];
        std::fs::write(dir.path().join("big.txt"), big).expect("write");
        commit_all(dir.path(), "big");
        let error = read_repo_bytes(
            &RepoRoot::new(dir.path()),
            &Rev::new("HEAD"),
            &rel("big.txt"),
        )
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
        let error = read_repo_bytes(
            &RepoRoot::new(dir.path()),
            &Rev::worktree(),
            &rel("leak.txt"),
        )
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
        let text = read_repo_text(
            &RepoRoot::new(dir.path()),
            &Rev::worktree(),
            &rel("alias.txt"),
        )
        .expect("inside");
        assert_eq!(text, "alpha note\n");
    }
}
