//! Sealed extractors for `file`, `table-column`, `subjects`, and `command`.
//!
//! No grammar crate is linked. `subjects` and `command` consume a
//! `sc/subjects/1` document that some other tool already wrote.

#![warn(missing_docs)]

mod command;
mod file;
mod read;
mod sanitize;
mod subjects;
mod table;

mod private {
    /// Marker that blocks extractor implementations outside this crate.
    pub trait Sealed {}
}

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

/// Phase A input extractor.
///
/// # Sealed
///
/// This trait is sealed and cannot be implemented outside `sc-jev-check`.
/// The only implementors are `file`, `table-column`, `subjects`, and `command`.
pub trait Extractor: private::Sealed {
    /// Stable kind name stored in `check.toml`.
    fn kind(&self) -> &'static str;

    /// Runs the extractor against `input`.
    ///
    /// # Errors
    ///
    /// Returns [`JevError`] when the path, document, or command result is
    /// unusable. A command input's `{path}` placeholder has no value here.
    fn extract(
        &self,
        repo: &RepoRoot,
        rev: &Rev,
        input: &InputSpec,
    ) -> Result<Vec<Subject>, JevError>;
}

pub(crate) use command::CommandExtractor;
pub(crate) use file::FileExtractor;
pub(crate) use read::{read_repo_bytes, read_repo_text, repo_path};
pub(crate) use subjects::SubjectsExtractor;
pub(crate) use table::TableColumnExtractor;

/// Runs the extractor selected by `input.kind`.
///
/// # Errors
///
/// Returns the extractor's [`JevError`]. Command inputs do not receive a
/// `{path}` value here, so `{path}` in their argv is `CONFIG_INVALID`.
pub fn extract_input(repo: &RepoRoot, rev: &Rev, input: &InputSpec) -> Result<Extracted, JevError> {
    match input {
        InputSpec::File { .. } => file::extract_file(repo, rev, input),
        InputSpec::TableColumn { .. } => table::extract_table(repo, rev, input),
        InputSpec::Subjects { .. } => subjects::extract_subjects(repo, rev, input),
        InputSpec::Command { .. } => CommandExtractor.extract_with_source(repo, rev, input, None),
    }
}

/// Returns the static extractor for `input`.
#[must_use]
pub fn extractor_for(input: &InputSpec) -> &'static dyn Extractor {
    match input {
        InputSpec::File { .. } => &FileExtractor,
        InputSpec::TableColumn { .. } => &TableColumnExtractor,
        InputSpec::Subjects { .. } => &SubjectsExtractor,
        InputSpec::Command { .. } => &CommandExtractor,
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use crate::deadline::spawn_with_deadline;

    pub use tempfile::TempDir;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extractors_are_file_table_subjects_and_command() {
        let samples = [
            InputSpec::File {
                path: "a.txt".into(),
            },
            InputSpec::TableColumn {
                path: "a.md".into(),
                column: "Choice".into(),
            },
            InputSpec::Subjects {
                path: "subjects.json".into(),
            },
            InputSpec::Command {
                command: vec!["true".into()],
                timeout_seconds: 30,
            },
        ];
        let kinds: Vec<&str> = samples
            .iter()
            .map(|input| extractor_for(input).kind())
            .collect();
        assert_eq!(kinds, ["file", "table-column", "subjects", "command"]);
    }
}
