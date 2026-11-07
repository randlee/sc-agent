//! Git blob reads for the `sc-treesitter` binary.
//!
//! The library never calls git. This module is part of the binary only.
//!
//! A revision read resolves `<rev>:<path>` to a blob id with
//! `git rev-parse --verify --end-of-options`, checks that the object is a
//! blob and that `git cat-file -s <id>` is within [`MAX_INPUT_BYTES`], then
//! reads `git cat-file blob <id>`. A `WORKTREE` read checks the file length
//! and reads at most [`MAX_INPUT_BYTES`] + 1 bytes. The bytes and the id therefore
//! come from the same object even if a ref moves between calls. Failures are
//! classified by exit status plus `git cat-file -e`, never by git's English
//! stderr. Every git call runs with `LC_ALL=C` and `GIT_TERMINAL_PROMPT=0`.
//!
//! A `WORKTREE` read hashes the bytes it returns with
//! `git hash-object -t blob --stdin --no-filters`, so the id is the blob id
//! of exactly those bytes and git never sees a filesystem path.
//!
//! Path rules match `sc-jev-check`'s repo-relative reads: no absolute path
//! after resolution, no `..`, no backslash, no `:`, no NUL, and no drive
//! prefix. An absolute argument is canonicalized (through symlinks such as
//! macOS `/tmp`) and must sit under the canonical repository toplevel.

use std::ffi::OsStr;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output, Stdio};

use sc_agent_types::{JevError, Rev};

/// Largest blob the binary reads (RSH-007).
///
/// Same 8 MiB value as `sc-jev-check`'s `MAX_INPUT_BYTES`; this crate does
/// not depend on `sc-jev-check`, so the value is repeated here. Larger
/// inputs are [`JevError::InputTooLarge`] before they are buffered.
pub(crate) const MAX_INPUT_BYTES: u64 = 8 * 1024 * 1024;

/// One blob read from the repository.
pub(crate) struct Blob {
    /// Repo-relative path with `/` separators.
    pub(crate) path: String,
    /// Blob bytes exactly as stored (no textconv, no filters).
    pub(crate) bytes: Vec<u8>,
    /// Lowercase hex object id (40 for SHA-1, 64 for SHA-256 repositories).
    pub(crate) sha: String,
}

/// Reads `path_arg` at `rev` from the repository containing the current directory.
///
/// # Errors
///
/// [`JevError::InputInvalid`] for a path outside the repository or with a
/// forbidden component, a revision git cannot resolve, a non-blob object, or
/// a git process that cannot be started. [`JevError::InputMissing`] when the
/// path is absent at `rev`. [`JevError::ExtractionFailed`] for any other git
/// failure.
pub(crate) fn read_blob(rev: &Rev, path_arg: &str) -> Result<Blob, JevError> {
    let repo = repo_root()?;
    let path = repo_relative(&repo, path_arg)?;
    if rev.as_str() == Rev::WORKTREE {
        return read_worktree(&repo, path);
    }
    read_at_rev(&repo, rev, path)
}

fn read_worktree(repo: &Path, path: String) -> Result<Blob, JevError> {
    let full = match std::fs::canonicalize(repo.join(&path)) {
        Ok(full) => full,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(JevError::InputMissing { path });
        }
        Err(error) => return Err(JevError::io_failed(&path, &error)),
    };
    if !full.starts_with(repo) {
        return Err(JevError::InputInvalid {
            detail: format!("{path} resolves outside the repository"),
        });
    }
    let metadata = std::fs::metadata(&full).map_err(|error| JevError::io_failed(&path, &error))?;
    if !metadata.is_file() {
        return Err(JevError::InputInvalid {
            detail: format!("{path} is not a regular file"),
        });
    }
    too_large(metadata.len())?;
    let file = std::fs::File::open(&full).map_err(|error| JevError::io_failed(&path, &error))?;
    let mut bytes = Vec::new();
    file.take(MAX_INPUT_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| JevError::io_failed(&path, &error))?;
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
fn hash_bytes(repo: &Path, bytes: &[u8], path: &str) -> Result<String, JevError> {
    let mut child = git_command()
        .arg("-C")
        .arg(repo)
        .args(["hash-object", "-t", "blob", "--stdin", "--no-filters"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| JevError::io_failed("git process spawn", &error))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| JevError::ExtractionFailed {
            detail: format!("git hash-object stdin unavailable for {path}"),
        })?;
    // Write on the current thread is safe: hash-object reads all of stdin
    // before it prints the 40/64-byte id, so stdout cannot fill first.
    let written = stdin.write_all(bytes);
    drop(stdin);
    let hashed = child
        .wait_with_output()
        .map_err(|error| JevError::io_failed("git hash-object", &error))?;
    if written.is_err() || !hashed.status.success() {
        return Err(JevError::ExtractionFailed {
            detail: format!("git hash-object failed for {path}"),
        });
    }
    object_id(&hashed.stdout, "git hash-object")
}

fn read_at_rev(repo: &Path, rev: &Rev, path: String) -> Result<Blob, JevError> {
    let spec = format!("{}:{path}", rev.as_str());
    let parsed = run_git(
        repo,
        [
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &spec,
        ],
    )?;
    if !parsed.status.success() {
        return Err(classify_unresolved(repo, rev, path)?);
    }
    let sha = object_id(&parsed.stdout, "git rev-parse")?;
    let kind = run_git(repo, ["cat-file", "-t", sha.as_str()])?;
    if !kind.status.success() {
        return Err(JevError::ExtractionFailed {
            detail: format!("git cat-file -t failed for {path} at {rev}"),
        });
    }
    if kind.stdout.trim_ascii() != b"blob" {
        return Err(JevError::InputInvalid {
            detail: format!("{path} is not a file at {rev}"),
        });
    }
    let size = run_git(repo, ["cat-file", "-s", sha.as_str()])?;
    let size = std::str::from_utf8(size.stdout.trim_ascii())
        .ok()
        .filter(|_| size.status.success())
        .and_then(|text| text.parse::<u64>().ok())
        .ok_or_else(|| JevError::ExtractionFailed {
            detail: format!("git cat-file -s failed for {path} at {rev}"),
        })?;
    too_large(size)?;
    let blob = run_git(repo, ["cat-file", "blob", sha.as_str()])?;
    if !blob.status.success() {
        return Err(JevError::ExtractionFailed {
            detail: format!("git cat-file blob failed for {path} at {rev}"),
        });
    }
    too_large(u64::try_from(blob.stdout.len()).unwrap_or(u64::MAX))?;
    Ok(Blob {
        path,
        bytes: blob.stdout,
        sha,
    })
}

/// [`JevError::InputTooLarge`] when `actual_bytes` exceeds [`MAX_INPUT_BYTES`].
fn too_large(actual_bytes: u64) -> Result<(), JevError> {
    if actual_bytes > MAX_INPUT_BYTES {
        return Err(JevError::InputTooLarge {
            limit_bytes: MAX_INPUT_BYTES,
            actual_bytes,
        });
    }
    Ok(())
}

/// Classifies a failed `<rev>:<path>` lookup without reading stderr.
///
/// If `<rev>^{tree}` exists the revision resolves and the path is missing;
/// otherwise the revision itself does not resolve.
fn classify_unresolved(repo: &Path, rev: &Rev, path: String) -> Result<JevError, JevError> {
    let tree = format!("{}^{{tree}}", rev.as_str());
    let exists = run_git(repo, ["cat-file", "-e", "--end-of-options", &tree])?;
    Ok(if exists.status.success() {
        JevError::InputMissing { path }
    } else {
        JevError::InputInvalid {
            detail: format!("revision {rev} was not found"),
        }
    })
}

/// Parses git's object-id output: 40 or 64 lowercase hex digits.
fn object_id(stdout: &[u8], source: &str) -> Result<String, JevError> {
    let id = stdout.trim_ascii();
    let well_formed = matches!(id.len(), 40 | 64)
        && id
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte));
    if !well_formed {
        return Err(JevError::ExtractionFailed {
            detail: format!("{source} returned an unexpected object id"),
        });
    }
    // Every byte is an ASCII hex digit, so the lossy conversion is exact.
    Ok(String::from_utf8_lossy(id).into_owned())
}

/// Turns `path_arg` into a validated repo-relative path with `/` separators.
///
/// `repo` must be canonical. An absolute `path_arg` is canonicalized with
/// [`canonicalize_lenient`] before the prefix check.
fn repo_relative(repo: &Path, path_arg: &str) -> Result<String, JevError> {
    let invalid = || JevError::InputInvalid {
        detail: format!("{path_arg} must be a repo-relative path inside the repository"),
    };
    let relative = if Path::new(path_arg).is_absolute() {
        let resolved = canonicalize_lenient(Path::new(path_arg))
            .map_err(|error| JevError::io_failed(path_arg, &error))?;
        let inside = resolved
            .strip_prefix(repo)
            .map_err(|_| JevError::InputInvalid {
                detail: format!("{path_arg} is outside the repository"),
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
    validate_relative(&relative).ok_or_else(invalid)
}

/// Applies `sc-jev-check`'s repo-relative rules and drops `.` components.
///
/// Returns `None` for empty text, a leading `/`, a backslash, NUL, `:`, `..`,
/// or any root or drive-prefix component.
fn validate_relative(relative: &str) -> Option<String> {
    if relative.is_empty()
        || relative.starts_with('/')
        || relative.contains(['\\', '\0', ':'])
        || Path::new(relative).is_absolute()
    {
        return None;
    }
    let mut parts = Vec::new();
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
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
fn repo_root() -> Result<PathBuf, JevError> {
    let output = git_command()
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|error| JevError::io_failed("git process spawn", &error))?;
    if !output.status.success() {
        return Err(JevError::ExtractionFailed {
            detail: "git rev-parse --show-toplevel failed; run sc-treesitter inside a repository"
                .into(),
        });
    }
    let text = String::from_utf8(output.stdout).map_err(|_| JevError::ExtractionFailed {
        detail: "git rev-parse returned a non-utf8 path".into(),
    })?;
    let toplevel = PathBuf::from(text.trim_end_matches(['\r', '\n']));
    std::fs::canonicalize(&toplevel).map_err(|error| JevError::io_failed("repository root", &error))
}

/// `git` with a fixed locale and no credential prompt.
fn git_command() -> Command {
    let mut command = Command::new("git");
    command
        .arg("--no-pager")
        .env("LC_ALL", "C")
        .env("GIT_TERMINAL_PROMPT", "0");
    command
}

fn run_git<I, S>(repo: &Path, args: I) -> Result<Output, JevError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    git_command()
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|error| JevError::io_failed("git process spawn", &error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_relative_matches_sc_jev_check_rules() {
        for bad in [
            "",
            "/etc/passwd",
            "..",
            "a/../b",
            "a\\b",
            "C:/x",
            "c:x",
            "a\0b",
            ".",
            "./",
        ] {
            assert_eq!(validate_relative(bad), None, "{bad:?} must be rejected");
        }
        assert_eq!(validate_relative("./src/a.py").as_deref(), Some("src/a.py"));
        assert_eq!(validate_relative("src//a.py").as_deref(), Some("src/a.py"));
    }

    #[test]
    fn object_id_accepts_sha1_and_sha256_lowercase_hex() {
        let sha1 = "a".repeat(40);
        let sha256 = "0123456789abcdef".repeat(4);
        assert_eq!(object_id(format!("{sha1}\n").as_bytes(), "t"), Ok(sha1));
        assert_eq!(object_id(sha256.as_bytes(), "t"), Ok(sha256));
        assert!(object_id("A".repeat(40).as_bytes(), "t").is_err());
        assert!(object_id("a".repeat(41).as_bytes(), "t").is_err());
        assert!(object_id(b"", "t").is_err());
    }

    #[test]
    fn canonicalize_lenient_keeps_absent_tail() {
        let dir = std::env::current_dir().expect("cwd");
        let resolved = canonicalize_lenient(&dir.join("absent-dir/absent.py")).expect("lenient");
        assert!(resolved.ends_with("absent-dir/absent.py"), "{resolved:?}");
        assert!(resolved.starts_with(std::fs::canonicalize(&dir).expect("canon")));
    }
}
