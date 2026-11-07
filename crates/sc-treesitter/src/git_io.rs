//! Source reads for the `sc-treesitter` binary.
//!
//! The library never calls git. This module is part of the binary only and
//! is compiled with the `cli` feature, which enables `sc-agent-types/git`.
//!
//! The repository toplevel comes from [`git::discover_toplevel`] and a
//! revision read from [`git::read_at_rev`] (RBQA-F006): argv-only git with
//! `--end-of-options`, `LC_ALL=C`, `GIT_TERMINAL_PROMPT=0`, a blob-only
//! check, and a read bounded by [`MAX_INPUT_BYTES`]. The bytes and the id
//! come from the same object even if a ref moves between calls.
//!
//! A `WORKTREE` read checks the file length, reads at most
//! [`MAX_INPUT_BYTES`] + 1 bytes, and hashes the bytes it returns with
//! `git hash-object -t blob --stdin --no-filters`, so the id is the blob id
//! of exactly those bytes and git never sees a filesystem path. The shared
//! module has no hashing helper, so this one git call stays here.
//!
//! Paths are validated by [`RepoRelativePath`] (the shared repo-relative
//! rules: no absolute path after resolution, no `..`, no backslash, no `:`,
//! no NUL) and normalized to `/`-joined normal components. An absolute
//! argument is canonicalized (through symlinks such as macOS `/tmp`) and
//! must sit under the canonical repository toplevel.

use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use sc_agent_types::git;
use sc_agent_types::{BlobSha, JevError, RepoRelativePath, RepoRoot, Rev};

/// Largest blob the binary reads (RSH-007).
///
/// Same 8 MiB value as `sc-jev-check`'s `MAX_INPUT_BYTES`; this crate does
/// not depend on `sc-jev-check`, so the value is repeated here. Larger
/// inputs are [`JevError::InputTooLarge`] before they are buffered.
pub(crate) const MAX_INPUT_BYTES: u64 = 8 * 1024 * 1024;

/// One blob read from the repository.
pub(crate) struct Blob {
    /// Normalized repo-relative path with `/` separators.
    pub(crate) path: RepoRelativePath,
    /// Blob bytes exactly as stored (no textconv, no filters).
    pub(crate) bytes: Vec<u8>,
    /// Object id (40 hex digits for SHA-1, 64 for SHA-256 repositories).
    pub(crate) sha: BlobSha,
}

/// Reads `path_arg` at `rev` from the repository containing the current directory.
///
/// # Errors
///
/// [`JevError::InputInvalid`] for a path outside the repository or with a
/// forbidden component, a revision git cannot resolve, a non-blob object, or
/// a git process that cannot be started. [`JevError::InputMissing`] when the
/// path is absent at `rev`. [`JevError::InputTooLarge`] above
/// [`MAX_INPUT_BYTES`]. [`JevError::ExtractionFailed`] outside a repository
/// or for any other git failure.
pub(crate) fn read_blob(rev: &Rev, path_arg: &str) -> Result<Blob, JevError> {
    let repo = repo_root()?;
    let path = repo_relative(repo.as_path(), path_arg)?;
    if rev.as_str() == Rev::WORKTREE {
        return read_worktree(&repo, path);
    }
    let blob = git::read_at_rev(&repo, rev, &path, MAX_INPUT_BYTES)?;
    Ok(Blob {
        path,
        sha: blob.sha().clone(),
        bytes: blob.into_bytes(),
    })
}

fn read_worktree(repo: &RepoRoot, path: RepoRelativePath) -> Result<Blob, JevError> {
    let full = match std::fs::canonicalize(repo.as_path().join(path.as_path())) {
        Ok(full) => full,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(JevError::input_missing(path.as_str()));
        }
        Err(error) => return Err(JevError::io_failed(path.as_str(), &error)),
    };
    if !full.starts_with(repo.as_path()) {
        return Err(JevError::input_invalid(format!(
            "{path} resolves outside the repository"
        )));
    }
    let metadata =
        std::fs::metadata(&full).map_err(|error| JevError::io_failed(path.as_str(), &error))?;
    if !metadata.is_file() {
        return Err(JevError::input_invalid(format!(
            "{path} is not a regular file"
        )));
    }
    too_large(metadata.len())?;
    let file =
        std::fs::File::open(&full).map_err(|error| JevError::io_failed(path.as_str(), &error))?;
    let mut bytes = Vec::new();
    file.take(MAX_INPUT_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| JevError::io_failed(path.as_str(), &error))?;
    too_large(u64::try_from(bytes.len()).unwrap_or(u64::MAX))?;
    let sha = hash_bytes(repo, &bytes, &path)?;
    Ok(Blob { path, bytes, sha })
}

/// Hashes `bytes` as a blob with `git hash-object -t blob --stdin --no-filters`.
///
/// Hashing the bytes already read (not the path) keeps the id and the
/// returned bytes in step even if the file changes between calls, skips clean
/// filters (`autocrlf`, attributes) so the id is that of the raw bytes, and
/// never hands git a platform path (Windows `canonicalize` yields `\\?\`
/// verbatim paths git cannot open). The repository's object format (SHA-1 or
/// SHA-256) applies because the command runs inside `repo`.
fn hash_bytes(repo: &RepoRoot, bytes: &[u8], path: &RepoRelativePath) -> Result<BlobSha, JevError> {
    let mut child = git_command(repo)
        .args(["hash-object", "-t", "blob", "--stdin", "--no-filters"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| JevError::io_failed("git process spawn", &error))?;
    let mut stdin = child.stdin.take().ok_or_else(|| {
        JevError::extraction_failed(format!("git hash-object stdin unavailable for {path}"))
    })?;
    // Write on the current thread is safe: hash-object reads all of stdin
    // before it prints the 40/64-byte id, so stdout cannot fill first.
    let written = stdin.write_all(bytes);
    drop(stdin);
    let hashed = child
        .wait_with_output()
        .map_err(|error| JevError::io_failed("git hash-object", &error))?;
    if written.is_err() || !hashed.status.success() {
        return Err(JevError::extraction_failed(format!(
            "git hash-object failed for {path}"
        )));
    }
    std::str::from_utf8(hashed.stdout.trim_ascii())
        .ok()
        .and_then(|text| BlobSha::try_from(text).ok())
        .ok_or_else(|| {
            JevError::extraction_failed("git hash-object returned an unexpected object id")
        })
}

/// [`JevError::InputTooLarge`] when `actual_bytes` exceeds [`MAX_INPUT_BYTES`].
fn too_large(actual_bytes: u64) -> Result<(), JevError> {
    if actual_bytes > MAX_INPUT_BYTES {
        return Err(JevError::input_too_large(MAX_INPUT_BYTES, actual_bytes));
    }
    Ok(())
}

/// Turns `path_arg` into a validated, normalized [`RepoRelativePath`].
///
/// `repo` must be canonical. An absolute `path_arg` is canonicalized with
/// [`canonicalize_lenient`] before the prefix check.
fn repo_relative(repo: &Path, path_arg: &str) -> Result<RepoRelativePath, JevError> {
    let invalid = || {
        JevError::input_invalid(format!(
            "{path_arg} must be a repo-relative path inside the repository"
        ))
    };
    let relative = if Path::new(path_arg).is_absolute() {
        let resolved = canonicalize_lenient(Path::new(path_arg))
            .map_err(|error| JevError::io_failed(path_arg, &error))?;
        let inside = resolved.strip_prefix(repo).map_err(|_| {
            JevError::input_invalid(format!("{path_arg} is outside the repository"))
        })?;
        let mut parts = Vec::new();
        for component in inside.components() {
            let Component::Normal(part) = component else {
                return Err(invalid());
            };
            parts.push(part.to_str().ok_or_else(invalid)?);
        }
        parts.join("/")
    } else {
        path_arg.to_owned()
    };
    let checked = RepoRelativePath::try_from(relative.as_str()).map_err(|_| invalid())?;
    normalize(&checked).ok_or_else(invalid)
}

/// Drops `.` and empty components of an already validated path, so
/// `./src//a.py` is recorded as `src/a.py`.
fn normalize(path: &RepoRelativePath) -> Option<RepoRelativePath> {
    let mut parts = Vec::new();
    for component in path.as_path().components() {
        if let Component::Normal(part) = component {
            parts.push(part.to_str()?);
        }
    }
    RepoRelativePath::try_from(parts.join("/")).ok()
}

/// Canonicalizes `path`, or its nearest existing ancestor plus the rest.
///
/// A file absent from the working tree (deleted, or present only at an older
/// revision) still resolves through a symlinked parent directory.
fn canonicalize_lenient(path: &Path) -> io::Result<PathBuf> {
    let mut tail = Vec::new();
    let mut current = path;
    loop {
        match std::fs::canonicalize(current) {
            Ok(mut base) => {
                for part in tail.iter().rev() {
                    base.push(part);
                }
                return Ok(base);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let (Some(parent), Some(name)) = (current.parent(), current.file_name()) else {
                    return Err(error);
                };
                tail.push(name.to_owned());
                current = parent;
            }
            Err(error) => return Err(error),
        }
    }
}

/// Canonical toplevel of the repository containing the current directory.
///
/// `sc-ai`'s `resolve_repo` runs the same lookup with a different error
/// mapping (QA-007): it can tell the caller to pass `--repo-root`
/// (`CONFIG_INVALID`); this producer has no such flag, so a failure is
/// `EXTRACTION_FAILED`. Both use [`git::discover_toplevel`] (decision D2).
fn repo_root() -> Result<RepoRoot, JevError> {
    let cwd = std::env::current_dir()
        .map_err(|error| JevError::io_failed("current directory", &error))?;
    let toplevel = git::discover_toplevel(&cwd)?.ok_or_else(|| {
        JevError::extraction_failed(
            "git rev-parse --show-toplevel failed; run sc-treesitter inside a repository",
        )
    })?;
    std::fs::canonicalize(toplevel.as_path())
        .map(RepoRoot::new)
        .map_err(|error| JevError::io_failed("repository root", &error))
}

/// `git -C <repo>` for `hash-object`, with the same environment policy as
/// [`sc_agent_types::git`]: fixed locale, no prompt, and no inherited
/// variable that would point git at another repository.
fn git_command(repo: &RepoRoot) -> Command {
    let mut command = Command::new("git");
    command
        .arg("--no-pager")
        .arg("-C")
        .arg(repo.as_path())
        .env("LC_ALL", "C")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES");
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_relative_applies_the_shared_rules() {
        let repo = Path::new("/nonexistent-sc-treesitter-repo");
        for bad in ["", "..", "a/../b", "a\\b", "C:/x", "c:x", "a\0b", ".", "./"] {
            assert_eq!(
                repo_relative(repo, bad).map_err(|error| error.code()),
                Err(sc_agent_types::ErrorCode::InputInvalid),
                "{bad:?} must be rejected"
            );
        }
        for (arg, expected) in [("./src/a.py", "src/a.py"), ("src//a.py", "src/a.py")] {
            let path = repo_relative(repo, arg).expect("valid path");
            assert_eq!(path, expected, "{arg:?}");
        }
    }

    #[test]
    fn canonicalize_lenient_keeps_absent_tail() {
        let dir = std::env::current_dir().expect("cwd");
        let resolved = canonicalize_lenient(&dir.join("absent-dir/absent.py")).expect("lenient");
        assert!(resolved.ends_with("absent-dir/absent.py"), "{resolved:?}");
        assert!(resolved.starts_with(std::fs::canonicalize(&dir).expect("canon")));
    }
}
