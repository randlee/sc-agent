//! Sealed extractors for `file`, `table-column`, `subjects`, and `command`.
//!
//! No grammar crate is linked. `subjects` and `command` consume a
//! `sc/subjects/1` document that some other tool already wrote.

#![warn(missing_docs)]

mod command;
mod file;
mod read;
mod subjects;
mod table;

mod private {
    /// Marker that blocks extractor implementations outside this crate.
    pub trait Sealed {}
}

use sc_agent_types::{InputSpec, JevError, RepoRoot, Rev, Subject, SubjectsDocument};

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
    /// unusable. Command `{path}` substitution uses [`CommandExtractor::extract_with_source`].
    fn extract(
        &self,
        repo: &RepoRoot,
        rev: &Rev,
        input: &InputSpec,
    ) -> Result<Vec<Subject>, JevError>;
}

#[doc(inline)]
pub use command::CommandExtractor;
#[doc(inline)]
pub use file::FileExtractor;
#[doc(inline)]
pub use read::{read_repo_bytes, read_repo_text};
#[doc(inline)]
pub use subjects::{SubjectsExtractor, parse_subjects_bytes, validate_subjects_document};
#[doc(inline)]
pub use table::TableColumnExtractor;

/// Runs the extractor selected by `input.kind`.
///
/// # Errors
///
/// Returns the extractor's [`JevError`]. Command inputs do not receive a
/// `{path}` value here; pass that through [`CommandExtractor::extract_with_source`].
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
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    pub struct TempDir(PathBuf);

    impl TempDir {
        pub fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    pub fn temp_dir(label: &str) -> TempDir {
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("sc-jev-check-{label}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).expect("temp dir");
        TempDir(path)
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
        git(
            dir.path(),
            &[
                "-c",
                "commit.gpgsign=false",
                "-c",
                "user.email=jev-check@example.com",
                "-c",
                "user.name=jev-check",
                "init",
            ],
        );
        git(dir.path(), &["add", "."]);
        git(
            dir.path(),
            &[
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "user.email=jev-check@example.com",
                "-c",
                "user.name=jev-check",
                "commit",
                "-m",
                "init",
            ],
        );
        let sha = git_stdout(dir.path(), &["rev-parse", "HEAD"]);
        (dir, sha.trim().to_owned())
    }

    pub fn python() -> &'static str {
        if cfg!(windows) { "python" } else { "python3" }
    }

    fn git(cwd: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(cwd)
            .args(args)
            .output()
            .expect("git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn git_stdout(cwd: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(cwd)
            .args(args)
            .output()
            .expect("git output");
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
