//! Subprocess helpers for the CLI tests.
//!
//! Every subprocess goes through [`spawn_with_deadline`], which wraps the
//! workspace's one bounded runner (`tests/support/deadline.rs`), so a hung
//! child fails the test instead of stalling CI. Git runs with the global and
//! system config disabled and no terminal prompt.

#[path = "../../../../tests/support/deadline.rs"]
mod deadline;

use std::path::Path;
use std::process::{Command, Output};

/// Platform null device used for `GIT_CONFIG_GLOBAL`.
pub fn null_device() -> &'static str {
    if cfg!(windows) { "NUL" } else { "/dev/null" }
}

/// Isolates `command` from the user's git config and credential prompts.
pub fn isolate_git(command: &mut Command) -> &mut Command {
    command
        .env("GIT_CONFIG_GLOBAL", null_device())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
}

/// Runs `command` to completion within the shared deadline, no stdin.
pub fn spawn_with_deadline(command: &mut Command) -> Output {
    deadline::spawn_with_deadline(command, None)
}

/// Runs `git args` in `dir` with a fixed identity; panics on failure.
pub fn git(dir: &Path, args: &[&str]) -> Output {
    let mut command = Command::new("git");
    isolate_git(&mut command)
        .current_dir(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "sc-treesitter")
        .env("GIT_AUTHOR_EMAIL", "sc-treesitter@example.com")
        .env("GIT_COMMITTER_NAME", "sc-treesitter")
        .env("GIT_COMMITTER_EMAIL", "sc-treesitter@example.com");
    let output = spawn_with_deadline(&mut command);
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// Commits everything in `dir` with signing disabled.
pub fn commit_all(dir: &Path, message: &str) {
    git(dir, &["add", "-A"]);
    git(
        dir,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-m",
            message,
        ],
    );
}
