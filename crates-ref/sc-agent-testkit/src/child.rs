//! [`ChildGuard`]: a child process that is killed and reaped on drop, waited on with a guard.

use std::io::Read;
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Output};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use sc_agent_types::{CancelToken, Clock};

use crate::wait::{HANG_GUARD, join_within};

/// How often a bounded wait looks at the child. A deadline wait between two `try_wait`
/// calls, never a sleep.
const EXIT_POLL: Duration = Duration::from_millis(10);

/// A spawned child that never outlives its test: dropping the guard kills and reaps the child
/// if it is still running, so a failing test leaves no orphan process.
#[derive(Debug)]
pub struct ChildGuard {
    child: Option<Child>,
}

impl ChildGuard {
    /// Spawns `command`.
    ///
    /// # Panics
    ///
    /// When the command cannot be spawned.
    #[track_caller]
    pub fn spawn(command: &mut Command) -> Self {
        let child = command
            .spawn()
            .unwrap_or_else(|error| panic!("could not spawn {command:?}: {error}"));
        Self { child: Some(child) }
    }

    /// The child's process id.
    #[must_use]
    pub fn id(&self) -> u32 {
        self.child.as_ref().map_or(0, Child::id)
    }

    /// The child's stdin pipe, taken (when it was piped).
    pub fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.as_mut().and_then(|child| child.stdin.take())
    }

    /// The child's stdout pipe, taken (when it was piped).
    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.as_mut().and_then(|child| child.stdout.take())
    }

    /// Waits for the child to exit, at most `guard`, and collects what it wrote to its piped
    /// stdout and stderr.
    ///
    /// # Panics
    ///
    /// When the child is still running after `guard` (it is then killed and reaped first), or
    /// its output cannot be read.
    #[track_caller]
    pub fn wait_with_output(mut self, guard: Duration) -> Output {
        let Some(mut child) = self.child.take() else {
            panic!("the child was already reaped");
        };
        let stdout = child.stdout.take().map(drain);
        let stderr = child.stderr.take().map(drain);
        let status = match exit_within(&mut child, guard) {
            Some(status) => status,
            None => {
                let pid = child.id();
                reap(&mut child);
                panic!("child {pid} did not exit within {guard:?}; it was killed");
            }
        };
        Output {
            status,
            stdout: stdout.map(collect).unwrap_or_default(),
            stderr: stderr.map(collect).unwrap_or_default(),
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            reap(&mut child);
        }
    }
}

/// Kills `child` if it still runs, then waits for it (a killed child exits at once).
fn reap(child: &mut Child) {
    if matches!(child.try_wait(), Ok(None)) {
        let _ = child.kill();
    }
    let _ = child.wait();
}

/// The exit status of `child` once it exits within `guard`; `None` if it is still running.
fn exit_within(child: &mut Child, guard: Duration) -> Option<ExitStatus> {
    let clock = Clock::system();
    let never = CancelToken::new();
    let deadline = clock.now() + guard;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {}
            Err(error) => panic!("could not wait for child {}: {error}", child.id()),
        }
        let now = clock.now();
        if now >= deadline {
            return None;
        }
        let _ = clock.wait_until(&never, (now + EXIT_POLL).min(deadline));
    }
}

/// Reads `pipe` to its end on its own thread.
fn drain(mut pipe: impl Read + Send + 'static) -> JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        bytes
    })
}

/// The bytes a [`drain`] thread read; the pipe closes when the child exits.
#[track_caller]
fn collect(reader: JoinHandle<Vec<u8>>) -> Vec<u8> {
    join_within(reader, HANG_GUARD).unwrap_or_else(|_| panic!("a pipe reader panicked"))
}

#[cfg(test)]
mod tests {
    use std::process::Stdio;

    use super::*;

    /// The test binary itself is a child that runs and exits (`--list` prints its tests).
    fn this_binary() -> Command {
        let mut command = Command::new(std::env::current_exe().expect("test binary"));
        command
            .arg("--list")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    #[test]
    fn a_child_that_exits_gives_its_output() {
        let output = ChildGuard::spawn(&mut this_binary()).wait_with_output(HANG_GUARD);
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("a_child_that_exits"));
    }

    #[test]
    fn a_dropped_guard_reaps_its_child() {
        let guard = ChildGuard::spawn(&mut this_binary());
        assert_ne!(guard.id(), 0);
        drop(guard);
    }
}
