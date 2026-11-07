//! `file` extractor.

use crate::config::InputSpec;
use sc_agent_types::schema::subjects::{Span, Subject};
use sc_agent_types::{JevError, RepoRoot, Rev};

use super::Extracted;
use super::Extractor;
use super::private::Sealed;
use super::read::read_repo_text;

/// Reads one repo-relative file into a single subject.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FileExtractor;

impl Sealed for FileExtractor {}

impl Extractor for FileExtractor {
    fn kind(&self) -> &'static str {
        "file"
    }

    fn extract(
        &self,
        repo: &RepoRoot,
        rev: &Rev,
        input: &InputSpec,
    ) -> Result<Vec<Subject>, JevError> {
        Ok(extract_file(repo, rev, input)?.subjects)
    }
}

pub(crate) fn extract_file(
    repo: &RepoRoot,
    rev: &Rev,
    input: &InputSpec,
) -> Result<Extracted, JevError> {
    let InputSpec::File { path } = input else {
        return Err(JevError::config_invalid(
            "file extractor received a different input kind",
        ));
    };
    let text = read_repo_text(repo, rev, path)?;
    if text.is_empty() {
        return Err(JevError::InputEmpty);
    }
    let end_byte = u32::try_from(text.len())
        .map_err(|_| JevError::input_too_large(u64::from(u32::MAX), text.len() as u64))?;
    let end_line = u32::try_from(text.lines().count().max(1)).unwrap_or(u32::MAX);
    Ok(Extracted {
        subjects: vec![Subject::new(
            format!("{path}:1"),
            "file",
            text,
            Span::new(1, end_line, 0, end_byte),
        )],
        document: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::InputSpec;
    use crate::extract::test_support::{git_fixture, temp_dir};

    #[test]
    fn extractors_file_reads_git_rev_not_the_worktree() {
        let (dir, sha) = git_fixture();
        std::fs::write(dir.path().join("notes/readme.txt"), "worktree edit\n").expect("edit");
        let repo = RepoRoot::new(dir.path());
        let input = InputSpec::File {
            path: "notes/readme.txt".into(),
        };
        let head = FileExtractor
            .extract(&repo, &Rev::new(&sha), &input)
            .expect("head");
        let worktree = FileExtractor
            .extract(&repo, &Rev::worktree(), &input)
            .expect("worktree");
        assert_eq!(head[0].text(), "alpha note\n");
        assert_eq!(worktree[0].text(), "worktree edit\n");
        assert_eq!(head[0].id(), "notes/readme.txt:1");
    }

    #[test]
    fn extractors_file_resolves_against_repo_root() {
        let dir = temp_dir("root");
        std::fs::write(dir.path().join("note.txt"), "from-root\n").expect("write");
        let repo = RepoRoot::new(dir.path());
        let input = InputSpec::File {
            path: "Cargo.toml".into(),
        };
        let error = FileExtractor
            .extract(&repo, &Rev::worktree(), &input)
            .expect_err("missing");
        assert_eq!(error.code(), "INPUT_MISSING");
        let found = FileExtractor
            .extract(
                &repo,
                &Rev::worktree(),
                &InputSpec::File {
                    path: "note.txt".into(),
                },
            )
            .expect("note");
        assert_eq!(found[0].text(), "from-root\n");
    }

    #[test]
    fn extractors_file_rejects_parent_and_absolute_paths() {
        let dir = temp_dir("escape");
        let absolute_dir = temp_dir("absolute");
        let absolute = absolute_dir.path().join("note.txt");
        let absolute = absolute.to_str().expect("temp path is utf-8");
        let repo = RepoRoot::new(dir.path());
        for path in ["../note.txt", absolute, ""] {
            let error = FileExtractor
                .extract(
                    &repo,
                    &Rev::worktree(),
                    &InputSpec::File { path: path.into() },
                )
                .expect_err(path);
            assert_eq!(error.code(), "INPUT_INVALID", "{path}");
        }
    }
}
