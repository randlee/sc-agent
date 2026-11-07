//! `file` extractor.

use sc_agent_types::schema::subjects::{Span, Subject};
use sc_agent_types::{JevError, RepoRelativePath, RepoRoot, Rev};

use super::Extracted;
use super::read::read_repo_text;

/// Reads one repo-relative file into a single subject.
pub(crate) fn extract_file(
    repo: &RepoRoot,
    rev: &Rev,
    path: &RepoRelativePath,
) -> Result<Extracted, JevError> {
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
    use crate::extract::extract_input;
    use crate::extract::test_support::{git_fixture, rel, temp_dir};

    #[test]
    fn extractors_file_reads_git_rev_not_the_worktree() {
        let (dir, sha) = git_fixture();
        std::fs::write(dir.path().join("notes/readme.txt"), "worktree edit\n").expect("edit");
        let repo = RepoRoot::new(dir.path());
        let path = rel("notes/readme.txt");
        let head = extract_file(&repo, &Rev::new(&sha), &path).expect("head");
        let worktree = extract_file(&repo, &Rev::worktree(), &path).expect("worktree");
        assert_eq!(head.subjects[0].text(), "alpha note\n");
        assert_eq!(worktree.subjects[0].text(), "worktree edit\n");
        assert_eq!(head.subjects[0].id(), "notes/readme.txt:1");
    }

    #[test]
    fn extractors_file_resolves_against_repo_root() {
        let dir = temp_dir("root");
        std::fs::write(dir.path().join("note.txt"), "from-root\n").expect("write");
        let repo = RepoRoot::new(dir.path());
        let error = extract_file(&repo, &Rev::worktree(), &rel("Cargo.toml")).expect_err("missing");
        assert_eq!(error.code(), "INPUT_MISSING");
        let found = extract_file(&repo, &Rev::worktree(), &rel("note.txt")).expect("note");
        assert_eq!(found.subjects[0].text(), "from-root\n");
        let dispatched = extract_input(
            &repo,
            &Rev::worktree(),
            &InputSpec::File {
                path: rel("note.txt"),
            },
        )
        .expect("extract_input dispatches file");
        assert_eq!(dispatched, found);
    }
}
