//! Subprocess helpers for the CLI tests.
//!
//! Copied from `sc-ai`'s `tests/cli.rs` (sharing test support across crates
//! is a Phase A.1 decision). Every subprocess goes through
//! [`spawn_with_deadline`], so a hung child fails the test after
//! [`SUBPROCESS_DEADLINE`] instead of stalling CI. Git runs with the global
//! config disabled and no terminal prompt.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Longest a subprocess may run before the test kills it.
pub const SUBPROCESS_DEADLINE: Duration = Duration::from_secs(60);

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

/// Runs `command` to completion within [`SUBPROCESS_DEADLINE`].
///
/// stdout and stderr drain on threads so a full pipe cannot block the child.
/// `try_wait` polls the deadline; on expiry the child is killed and reaped
/// and the test panics with the captured stderr.
pub fn spawn_with_deadline(command: &mut Command) -> Output {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn subprocess");
    let stdout = drain(child.stdout.take().expect("piped stdout"));
    let stderr = drain(child.stderr.take().expect("piped stderr"));
    let started = Instant::now();
    let exit = loop {
        match child.try_wait().expect("poll subprocess") {
            Some(exit) => break exit,
            None if started.elapsed() >= SUBPROCESS_DEADLINE => {
                let _ = child.kill();
                let _ = child.wait();
                let stderr = stderr.join().unwrap_or_default();
                panic!(
                    "subprocess exceeded {SUBPROCESS_DEADLINE:?}; stderr: {}",
                    String::from_utf8_lossy(&stderr)
                );
            }
            None => thread::sleep(Duration::from_millis(20)),
        }
    };
    Output {
        status: exit,
        stdout: stdout.join().expect("stdout reader"),
        stderr: stderr.join().expect("stderr reader"),
    }
}

fn drain(mut pipe: impl Read + Send + 'static) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        bytes
    })
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
