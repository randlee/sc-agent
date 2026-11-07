//! Argv-only git helpers (feature `git`, RBQA-F006).
//!
//! Every call runs `git --no-pager -C <dir> ...` with `GIT_TERMINAL_PROMPT=0`
//! and `LC_ALL=C`, passes revisions after `--end-of-options`, and classifies
//! failures by exit status and follow-up probes, never by parsing localized
//! messages. Blob bytes come from `git cat-file blob <sha>`, which writes the
//! stored bytes without textconv or filters, through a reader bounded by
//! the caller's byte limit.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use crate::error::JevError;
use crate::ids::{BlobSha, RepoRelativePath, RepoRoot, Rev};

/// One blob read from git: its object id and bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBlob {
    sha: BlobSha,
    bytes: Vec<u8>,
}

impl GitBlob {
    /// The blob's object id.
    #[must_use]
    pub fn sha(&self) -> &BlobSha {
        &self.sha
    }

    /// The blob bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the bytes, consuming the blob.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

fn git(dir: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .arg("--no-pager")
        .arg("-C")
        .arg(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::null());
    command
}

fn run(mut command: Command) -> Result<Output, JevError> {
    command
        .output()
        .map_err(|error| JevError::io_failed("git", &error))
}

/// Finds the git toplevel that contains `start`.
///
/// Returns `Ok(None)` when `start` is not inside a git work tree.
///
/// # Errors
///
/// `INPUT_INVALID` when git cannot be started or prints a toplevel that is
/// not UTF-8.
pub fn discover_toplevel(start: &Path) -> Result<Option<RepoRoot>, JevError> {
    let mut command = git(start);
    command.args(["rev-parse", "--show-toplevel"]);
    let output = run(command)?;
    if !output.status.success() {
        return Ok(None);
    }
    let text = std::str::from_utf8(&output.stdout)
        .map_err(|_| JevError::input_invalid("git printed a toplevel path that is not UTF-8"))?;
    let path = text.trim_end_matches(['\n', '\r']);
    if path.is_empty() {
        return Ok(None);
    }
    Ok(Some(RepoRoot::new(path)))
}

/// Reads `path` at `rev` from the repository at `root`.
///
/// Resolves `{rev}:{path}` with `rev-parse --verify` to an object id, then
/// reads it with `cat-file blob`, which rejects trees. At most `max_bytes`
/// bytes are read.
///
/// # Errors
///
/// - `INPUT_INVALID` when `rev` is [`Rev::WORKTREE`], starts with `-`, does
///   not resolve, or `path` names something other than a file;
/// - `INPUT_MISSING` when `rev` resolves but has no `path`;
/// - `INPUT_TOO_LARGE` when the blob is larger than `max_bytes`;
/// - `INPUT_INVALID` (`io_failed`) when git cannot be started or read.
pub fn read_at_rev(
    root: &RepoRoot,
    rev: &Rev,
    path: &RepoRelativePath,
    max_bytes: u64,
) -> Result<GitBlob, JevError> {
    let rev_text = rev.as_str();
    if rev_text == Rev::WORKTREE || rev_text.starts_with('-') || rev_text.is_empty() {
        return Err(JevError::input_invalid(format!(
            "revision {rev_text} is not a git revision that can be read"
        )));
    }
    let object = format!("{rev_text}:{}", path.as_str());
    let mut resolve = git(root.as_path());
    resolve.args([
        "rev-parse",
        "--verify",
        "--quiet",
        "--end-of-options",
        &object,
    ]);
    let resolved = run(resolve)?;
    if !resolved.status.success() {
        return Err(classify_missing(root, rev, path)?);
    }
    let sha_text = std::str::from_utf8(&resolved.stdout)
        .map(str::trim)
        .unwrap_or_default();
    let sha = BlobSha::try_from(sha_text).map_err(|_| {
        JevError::extraction_failed("git rev-parse returned an unexpected object id")
    })?;

    let mut kind = git(root.as_path());
    kind.args(["cat-file", "-t", sha.as_str()]);
    let kind = run(kind)?;
    if !kind.status.success() || kind.stdout.trim_ascii() != b"blob" {
        return Err(JevError::input_invalid(format!(
            "{path} at {rev} is not a file",
            path = path.as_str()
        )));
    }
    let mut size = git(root.as_path());
    size.args(["cat-file", "-s", sha.as_str()]);
    let size = run(size)?;
    let actual_bytes = std::str::from_utf8(&size.stdout)
        .ok()
        .and_then(|text| text.trim().parse::<u64>().ok());
    if let Some(actual_bytes) = actual_bytes
        && actual_bytes > max_bytes
    {
        return Err(JevError::input_too_large(max_bytes, actual_bytes));
    }

    let mut read = git(root.as_path());
    read.args(["cat-file", "blob", sha.as_str()])
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = read
        .spawn()
        .map_err(|error| JevError::io_failed("git", &error))?;
    let mut bytes = Vec::new();
    let limit = max_bytes.saturating_add(1);
    let read_result = child
        .stdout
        .take()
        .map(|stdout| stdout.take(limit).read_to_end(&mut bytes));
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        let _ = child.kill();
        let _ = child.wait();
        return Err(JevError::input_too_large(
            max_bytes,
            u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        ));
    }
    let exit = child
        .wait()
        .map_err(|error| JevError::io_failed("git", &error))?;
    match read_result {
        Some(Ok(_)) if exit.success() => Ok(GitBlob { sha, bytes }),
        Some(Err(error)) => Err(JevError::io_failed("git cat-file output", &error)),
        _ => Err(JevError::input_invalid(format!(
            "{path} at {rev} is not a file",
            path = path.as_str()
        ))),
    }
}

/// `INPUT_MISSING` when `rev` has a tree (the path is absent), otherwise
/// `INPUT_INVALID` (the revision does not resolve).
fn classify_missing(
    root: &RepoRoot,
    rev: &Rev,
    path: &RepoRelativePath,
) -> Result<JevError, JevError> {
    let tree = format!("{}^{{tree}}", rev.as_str());
    let mut probe = git(root.as_path());
    probe.args(["cat-file", "-e", "--end-of-options", &tree]);
    let exists = run(probe)?;
    Ok(if exists.status.success() {
        JevError::input_missing(path.as_str())
    } else {
        JevError::input_invalid(format!("revision {rev} was not found"))
    })
}
