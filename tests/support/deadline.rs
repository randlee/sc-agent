//! Bounded subprocess runner shared by the workspace tests (FTQ-003, FTQ-008,
//! FTQ-011).
//!
//! This is the one copy. Test code includes it with `#[path]`:
//! `sc-ai` and `sc-treesitter` integration tests, `sc-jev-check` unit tests,
//! and the `jev_cross_impl` workspace test. Sharing a dev-support crate is a
//! Phase A.1 decision; a single included file keeps one implementation now.
//!
//! Every wait is bounded:
//!
//! - stdin is written on its own thread, so a child that never reads it
//!   cannot block the runner;
//! - the child must exit within [`SUBPROCESS_DEADLINE`], or it is killed and
//!   the test panics with the stderr captured so far;
//! - stdout and stderr drain on threads that append to shared buffers. After
//!   the child exits (or is killed) the runner waits at most [`PIPE_GRACE`]
//!   for each pipe to close. A pipe still open then is held by a grandchild;
//!   the runner says so on stderr, kills the process group, and returns what
//!   it captured instead of joining the reader thread.
//!
//! On Unix the child leads its own process group and the runner kills the
//! whole group (`kill -KILL -- -<pgid>`), so grandchildren such as a git hook
//! or an `sh -c '... &'` job die with it. Windows kills only the child; the
//! bounded pipe wait still keeps the runner from hanging.

use std::io::{Read, Write};
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

/// Longest a subprocess may run before the test kills it.
pub const SUBPROCESS_DEADLINE: Duration = Duration::from_secs(60);

/// Longest the runner waits for each of stdout and stderr to close after the
/// child exits or is killed.
pub const PIPE_GRACE: Duration = Duration::from_secs(5);

/// Runs `command` to completion within [`SUBPROCESS_DEADLINE`].
///
/// `stdin` is written then closed; `None` attaches the null device.
///
/// # Panics
///
/// When the child cannot be spawned or the deadline passes.
pub fn spawn_with_deadline(command: &mut Command, stdin: Option<&[u8]>) -> Output {
    run_with_limits(command, stdin, SUBPROCESS_DEADLINE, PIPE_GRACE)
}

fn run_with_limits(
    command: &mut Command,
    stdin: Option<&[u8]>,
    deadline: Duration,
    grace: Duration,
) -> Output {
    command
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    own_process_group(command);
    let started = Instant::now();
    let mut child = command.spawn().expect("spawn subprocess");
    let stdout = Drain::start(child.stdout.take().expect("piped stdout"));
    let stderr = Drain::start(child.stderr.take().expect("piped stderr"));
    if let Some(bytes) = stdin {
        let mut pipe = child.stdin.take().expect("piped stdin");
        let bytes = bytes.to_vec();
        // Detached: a child that never reads stdin must not block the runner.
        // A write error means the child exited or closed stdin early.
        thread::spawn(move || {
            let _ = pipe.write_all(&bytes);
        });
    }
    let exit = loop {
        match child.try_wait().expect("poll subprocess") {
            Some(exit) => break Some(exit),
            None if started.elapsed() >= deadline => break None,
            None => thread::sleep(Duration::from_millis(20)),
        }
    };
    let Some(exit) = exit else {
        kill_tree(&mut child);
        let _ = child.wait();
        let captured = stderr.finish(grace, "stderr", &mut child);
        panic!(
            "subprocess exceeded {deadline:?}; stderr: {}",
            String::from_utf8_lossy(&captured)
        );
    };
    Output {
        status: exit,
        stdout: stdout.finish(grace, "stdout", &mut child),
        stderr: stderr.finish(grace, "stderr", &mut child),
    }
}

/// A pipe read on its own thread into a shared buffer.
struct Drain {
    bytes: Arc<Mutex<Vec<u8>>>,
    closed: Receiver<()>,
}

impl Drain {
    fn start(mut pipe: impl Read + Send + 'static) -> Self {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let (done, closed) = mpsc::channel();
        let sink = Arc::clone(&bytes);
        thread::spawn(move || {
            let mut chunk = [0_u8; 8192];
            loop {
                match pipe.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => sink
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .extend_from_slice(&chunk[..read]),
                }
            }
            let _ = done.send(());
        });
        Self { bytes, closed }
    }

    /// Waits up to `grace` for EOF, then returns what was read.
    ///
    /// A pipe still open after `grace` is held by a grandchild: the process
    /// group is killed (Unix) and the captured bytes are returned without
    /// joining the reader thread.
    fn finish(&self, grace: Duration, name: &str, child: &mut Child) -> Vec<u8> {
        if self.closed.recv_timeout(grace).is_err() {
            eprintln!(
                "{name} pipe held open by a grandchild after {grace:?}; using the output captured so far"
            );
            kill_tree(child);
        }
        self.bytes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

#[cfg(unix)]
fn own_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(not(unix))]
fn own_process_group(_command: &mut Command) {}

/// Kills the child and, on Unix, every process in its group.
fn kill_tree(child: &mut Child) {
    #[cfg(unix)]
    {
        // The child leads its group, so its pid is the group id. A group
        // that is already gone makes `kill` fail, which is fine.
        let _ = Command::new("kill")
            .args(["-KILL", "--", &format!("-{}", child.id())])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    const TEST_DEADLINE: Duration = Duration::from_millis(500);
    const TEST_GRACE: Duration = Duration::from_millis(500);

    // No wall-clock asserts (FTQ-002): each property is shown by what the
    // runner returns, so a loaded CI host cannot fail these tests.

    /// A grandchild that keeps stdout open after the child exits cannot
    /// stall the runner; the child's own output is still returned. The
    /// grandchild prints `late` only if it lives out its sleep, so seeing
    /// exactly `early` shows the runner returned (and killed the group)
    /// instead of waiting for the pipe to close.
    #[test]
    fn grandchild_holding_the_pipe_is_bounded_by_the_grace() {
        let output = run_with_limits(
            Command::new("sh").args(["-c", "(sleep 30; echo late) & echo early"]),
            None,
            Duration::from_secs(60),
            TEST_GRACE,
        );
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "early\n");
    }

    /// A hung child with a pipe-holding grandchild fails at the deadline, not
    /// when the grandchild exits: left alone the child would exit 0, so a
    /// panic shows the deadline fired.
    #[test]
    fn deadline_kill_returns_despite_a_grandchild() {
        let outcome = std::panic::catch_unwind(|| {
            run_with_limits(
                Command::new("sh").args(["-c", "sleep 30 & sleep 30"]),
                None,
                TEST_DEADLINE,
                TEST_GRACE,
            )
        });
        assert!(outcome.is_err(), "the deadline must fail the run");
    }

    /// A child that never reads stdin cannot block the runner on the write.
    #[test]
    fn unread_stdin_does_not_block() {
        let big = vec![b'x'; 4 * 1024 * 1024];
        let output = run_with_limits(
            Command::new("sh").args(["-c", "exit 0"]),
            Some(&big),
            Duration::from_secs(30),
            TEST_GRACE,
        );
        assert!(output.status.success());
    }
}
