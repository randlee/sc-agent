//! Argv-only git helpers (feature `git`, RBQA-F006). One temp repo per test.
//!
//! Needs `--features git` (`[[test]] required-features`, D0 outcome).
//!
//! Fixture git runs ignore the host's global and system config (no gpg
//! signing, hooks or templates) and go through the workspace's bounded
//! runner, so a stuck git fails the test instead of hanging it.

use std::path::Path;
use std::process::Command;

#[path = "../../../tests/support/deadline.rs"]
mod deadline;

/// The platform null device, used as an empty global git config.
fn null_device() -> &'static str {
    if cfg!(windows) { "NUL" } else { "/dev/null" }
}

use sc_agent_types::git::{discover_toplevel, read_at_rev};
use sc_agent_types::{RepoRelativePath, RepoRoot, Rev};

fn git(dir: &Path, args: &[&str]) {
    let hooks = format!("core.hooksPath={}", null_device());
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.email=t@example.com", "-c", "user.name=t"])
        .args(["-c", "commit.gpgsign=false", "-c", &hooks])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", null_device())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        // Same hook-inherited variables the production helper removes (FTQ-001).
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES");
    let output = deadline::spawn_with_deadline(&mut command, None);
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn repo() -> (tempfile::TempDir, RepoRoot) {
    let dir = tempfile::tempdir().expect("temp");
    git(dir.path(), &["init", "-q"]);
    std::fs::create_dir(dir.path().join("src")).expect("src");
    std::fs::write(dir.path().join("src/a.py"), b"x = 1\n\xff raw bytes\n").expect("file");
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "init"]);
    let root = discover_toplevel(dir.path())
        .expect("discover")
        .expect("inside a repo");
    (dir, root)
}

fn path(text: &str) -> RepoRelativePath {
    RepoRelativePath::try_from(text).expect("path")
}

#[test]
fn test_read_at_rev_matches_worktree_bytes() {
    let (dir, root) = repo();
    let blob = read_at_rev(&root, &Rev::new("HEAD"), &path("src/a.py"), 1024).expect("read");
    let worktree = std::fs::read(dir.path().join("src/a.py")).expect("worktree");
    assert_eq!(blob.bytes(), worktree.as_slice());
    assert_eq!(blob.sha().as_str().len(), 40);
    assert_eq!(blob.clone().into_bytes(), worktree);
    let missing =
        read_at_rev(&root, &Rev::new("HEAD"), &path("src/none.py"), 1024).expect_err("absent path");
    assert_eq!(missing.code(), "INPUT_MISSING");
}

#[test]
fn test_read_at_rev_rejects_option_injection() {
    let (dir, root) = repo();
    let target = dir.path().join("FILE");
    let rev = Rev::new(format!("--output={}", target.display()));
    let error = read_at_rev(&root, &rev, &path("src/a.py"), 1024).expect_err("option");
    assert_eq!(error.code(), "INPUT_INVALID");
    assert!(!target.exists(), "no file is created");
    let unknown = read_at_rev(&root, &Rev::new("no-such-branch"), &path("src/a.py"), 1024)
        .expect_err("unknown rev");
    assert_eq!(unknown.code(), "INPUT_INVALID");
    let worktree =
        read_at_rev(&root, &Rev::worktree(), &path("src/a.py"), 1024).expect_err("worktree");
    assert_eq!(worktree.code(), "INPUT_INVALID");
}

#[test]
fn test_read_at_rev_rejects_tree() {
    let (_dir, root) = repo();
    let error = read_at_rev(&root, &Rev::new("HEAD"), &path("src"), 1024).expect_err("tree");
    assert_eq!(error.code(), "INPUT_INVALID");
    assert!(
        error.fix_hint().contains("not a file"),
        "{}",
        error.fix_hint()
    );
}

#[test]
fn test_read_at_rev_bounded() {
    let (_dir, root) = repo();
    let error = read_at_rev(&root, &Rev::new("HEAD"), &path("src/a.py"), 4).expect_err("cap");
    assert_eq!(error.code(), "INPUT_TOO_LARGE");
    let exact = read_at_rev(&root, &Rev::new("HEAD"), &path("src/a.py"), 18).expect("fits");
    assert_eq!(exact.bytes().len(), 18);
}

#[test]
fn test_discover_toplevel_from_subdirectory() {
    let (dir, root) = repo();
    let from_sub = discover_toplevel(&dir.path().join("src"))
        .expect("discover")
        .expect("repo");
    assert_eq!(from_sub, root);
    let canonical = std::fs::canonicalize(dir.path()).expect("canonical");
    assert_eq!(
        std::fs::canonicalize(root.as_path()).expect("root"),
        canonical
    );
    let outside = tempfile::tempdir().expect("outside");
    assert_eq!(discover_toplevel(outside.path()).expect("not a repo"), None);
}
