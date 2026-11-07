//! Bounded subprocess runner shared by the workspace tests (FTQ-003, FTQ-008).
//!
//! This is the one copy. Test code includes it with `#[path]`:
//! `sc-ai` and `sc-treesitter` integration tests, `sc-jev-check` unit tests,
//! and the `jev_cross_impl` workspace test. Sharing a dev-support crate is a
//! Phase A.1 decision; a single included file keeps one implementation now.
//!
//! Every subprocess a test waits on goes through [`spawn_with_deadline`], so
//! a hung child fails the test after [`SUBPROCESS_DEADLINE`] with its stderr
//! instead of stalling CI until the job timeout.

use std::io::{Read, Write};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Longest a subprocess may run before the test kills it.
pub const SUBPROCESS_DEADLINE: Duration = Duration::from_secs(60);

/// Runs `command` to completion within [`SUBPROCESS_DEADLINE`].
///
/// `stdin` is written then closed; `None` attaches the null device. stdout
/// and stderr drain on threads so a full pipe cannot block the child.
/// `try_wait` polls the deadline; on expiry the child is killed and reaped
/// and the test panics with the captured stderr.
///
/// # Panics
///
/// When the child cannot be spawned, stdin cannot be written, or the
/// deadline passes.
pub fn spawn_with_deadline(command: &mut Command, stdin: Option<&[u8]>) -> Output {
    command
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn subprocess");
    let stdout = drain(child.stdout.take().expect("piped stdout"));
    let stderr = drain(child.stderr.take().expect("piped stderr"));
    if let Some(bytes) = stdin {
        let mut pipe = child.stdin.take().expect("piped stdin");
        pipe.write_all(bytes).expect("write stdin");
    }
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
