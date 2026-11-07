//! Extraction for the four `check.toml` input kinds.
//!
//! [`extract_input`] matches `file`, `table-column`, `subjects`, and
//! `command` directly; there is no extractor trait. A new kind is a new
//! [`InputSpec`] variant plus an ADR (Rust API patterns, r-2 amendment).
//! No grammar crate is linked. `subjects` and `command` consume a
//! `sc/subjects/1` document that some other tool already wrote.

#![warn(missing_docs)]

mod command;
mod file;
mod read;
mod sanitize;
mod subjects;
mod table;

use crate::config::InputSpec;
use sc_agent_types::schema::subjects::{Subject, SubjectsDocument};
use sc_agent_types::{JevError, RepoRoot, Rev};

/// Subjects produced by one input, plus the source document when there is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extracted {
    /// Mapped subject id and text values.
    pub subjects: Vec<Subject>,
    /// Present for `subjects` and `command` inputs.
    pub document: Option<SubjectsDocument>,
}

pub(crate) use read::{read_repo_bytes, read_repo_text, repo_path};

/// Runs the extraction selected by the input kind.
///
/// A `command` input's `{path}` placeholder takes the input's own `path`
/// field (REQ-SAG-0312); `{path}` without one is `CONFIG_INVALID`.
///
/// # Errors
///
/// Returns the [`JevError`] of the selected extraction: `INPUT_*` for an
/// unreadable path, `EXTRACTION_*` for a bad document or command result, or
/// the producer's own code when a command reports an error response on
/// stderr (REQ-SAG-0313).
pub fn extract_input(repo: &RepoRoot, rev: &Rev, input: &InputSpec) -> Result<Extracted, JevError> {
    match input {
        InputSpec::File { path } => file::extract_file(repo, rev, path),
        InputSpec::TableColumn { path, column } => table::extract_table(repo, rev, path, column),
        InputSpec::Subjects { path } => subjects::extract_subjects(repo, rev, path),
        InputSpec::Command {
            command,
            timeout_seconds,
            path,
        } => command::extract_command(repo, rev, command, *timeout_seconds, path.as_ref()),
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use crate::deadline::spawn_with_deadline;

    pub use tempfile::TempDir;

    /// A repo-relative path for tests.
    pub fn rel(text: &str) -> sc_agent_types::RepoRelativePath {
        sc_agent_types::RepoRelativePath::try_from(text).expect("repo-relative path")
    }

    pub fn temp_dir(label: &str) -> TempDir {
        tempfile::Builder::new()
            .prefix(&format!("sc-jev-check-{label}-"))
            .tempdir()
            .expect("temp dir")
    }

    pub fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/jev/git-repo")
    }

    pub fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    pub fn copy_fixture() -> TempDir {
        let dir = temp_dir("fixture");
        copy_tree(&fixture_root(), dir.path()).expect("copy fixture");
        dir
    }

    pub fn git_fixture() -> (TempDir, String) {
        let dir = copy_fixture();
        git(dir.path(), &["init", "-q"]);
        commit_all(dir.path(), "init");
        let sha = git_stdout(dir.path(), &["rev-parse", "HEAD"]);
        (dir, sha.trim().to_owned())
    }

    pub fn python() -> &'static str {
        if cfg!(windows) { "python" } else { "python3" }
    }

    /// Git with no global or system config, no terminal prompt, and a fixed
    /// identity.
    pub fn git_command(cwd: &Path) -> Command {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(cwd)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_CONFIG_GLOBAL", null_device())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "user.email=jev-check@example.com",
                "-c",
                "user.name=jev-check",
            ]);
        command
    }

    fn null_device() -> &'static str {
        if cfg!(windows) { "NUL" } else { "/dev/null" }
    }

    /// Stages everything under `cwd` and commits it.
    pub fn commit_all(cwd: &Path, message: &str) {
        git(cwd, &["add", "-A"]);
        git(cwd, &["commit", "-q", "-m", message]);
    }

    pub fn git(cwd: &Path, args: &[&str]) {
        let output = spawn_with_deadline(git_command(cwd).args(args), None);
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn git_stdout(cwd: &Path, args: &[&str]) -> String {
        let output = spawn_with_deadline(git_command(cwd).args(args), None);
        assert!(output.status.success(), "git {args:?} failed");
        String::from_utf8(output.stdout).expect("utf-8")
    }

    fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let to = dst.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                copy_tree(&entry.path(), &to)?;
            } else {
                std::fs::copy(entry.path(), to)?;
            }
        }
        Ok(())
    }
}
