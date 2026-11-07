//! `command` extractor (REQ-SAG-0305, REQ-SAG-0312, REQ-SAG-0313).
//!
//! Argv only: no shell. Placeholders `{path}`, `{rev}`, and `{repo_root}` are
//! replaced before spawn in one left-to-right pass. `{path}` is the input's
//! own `path` field from `check.toml`, validated like a `file` input path;
//! `{path}` with no `path` field is `CONFIG_INVALID`. `{repo_root}` is always
//! absolute. The working directory is the repo root.
//!
//! The child starts with a cleared environment plus an allowlist (`PATH`,
//! `HOME`, `USERPROFILE`, `SYSTEMROOT`, `TEMP`, `TMP`, `TMPDIR`, `LANG`,
//! `LC_*`, `TERM`), so provider API keys and `SC_*` settings in this process
//! never reach a repo-configured command.
//!
//! On a non-zero exit the last non-empty line of the child's stderr (bounded
//! by the 4 KiB stderr tail) is tried as an `ScAgentErrorResponse`. A
//! response with a known id becomes [`JevError::ProducerError`] carrying
//! that id and the producer name (the file stem of `argv[0]`); anything else
//! is `EXTRACTION_FAILED` with the stderr tail. Credential-looking values are
//! redacted from either error.

use std::collections::BTreeMap;
use std::io::Read;
use std::mem::ManuallyDrop;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use sc_agent_types::{JevError, RepoRelativePath, RepoRoot, Rev, ScAgentErrorResponse};

use super::Extracted;
use super::read::validate_input_path;
use super::sanitize::{STDERR_TAIL_BYTES, apply_env_allowlist, redact_error, stderr_tail};
use super::subjects::parse_subjects_bytes;
use crate::payload::expand_placeholders;

/// Largest stdout capture for one command.
///
/// Subjects documents are small. Stopping at 8 MiB bounds memory if a command
/// writes without exiting. Each backend still enforces its own request byte
/// limit on the assembled payload (REQ-SAG-0314).
const MAX_COMMAND_STDOUT_BYTES: usize = 8 * 1024 * 1024;

/// How often a running command is polled.
///
/// 20 ms keeps timeout tests short without a busy spin.
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Bound on killing a child and collecting its pipe readers.
///
/// Starts when the command's own timeout fires, or when the process has
/// already exited. Past this deadline the caller returns. `Child`'s
/// destructor and `JoinHandle::join` are not used: both wait without a limit.
const CLEANUP_BUDGET: Duration = Duration::from_secs(2);

/// Runs one `command` input and parses its stdout as `sc/subjects/1`.
///
/// `path` is the input's `path` field. It is validated with the `file`
/// input rules (repo-relative, a regular file whose canonical path stays
/// under the canonical repo root at `WORKTREE`, a blob at any other `rev`)
/// and substituted for `{path}`.
///
/// # Errors
///
/// Returns [`JevError::ConfigInvalid`] for an empty argv, a zero timeout, or
/// `{path}` without a `path` field; the `file` input errors for an invalid
/// `path`; [`JevError::ProducerError`] when the child exits non-zero and its
/// last stderr line is an error response with a known id;
/// [`JevError::ExtractionFailed`] on any other non-zero exit;
/// [`JevError::ExtractionTimeout`] when `timeout_seconds` elapses;
/// [`JevError::ExtractionInvalid`] when stdout is not `sc/subjects/1`; and
/// [`JevError::ExtractionEmpty`] when the document has no subjects.
pub(crate) fn extract_command(
    repo: &RepoRoot,
    rev: &Rev,
    command: &[String],
    timeout_seconds: u64,
    path: Option<&RepoRelativePath>,
) -> Result<Extracted, JevError> {
    if timeout_seconds == 0 {
        return Err(JevError::config_invalid(
            "command timeout_seconds must be greater than zero",
        ));
    }
    let argv = substitute_placeholders(command, repo, rev, path.map(RepoRelativePath::as_str))?;
    if let Some(path) = path {
        validate_input_path(repo, rev, path)?;
    }
    let stdout = run_argv(repo, &argv, Duration::from_secs(timeout_seconds))?;
    parse_subjects_bytes(&stdout)
}

pub(crate) fn substitute_placeholders(
    argv: &[String],
    repo: &RepoRoot,
    rev: &Rev,
    source_path: Option<&str>,
) -> Result<Vec<String>, JevError> {
    if argv.is_empty() || argv[0].is_empty() {
        return Err(JevError::config_invalid(
            "command argv must start with a program",
        ));
    }
    let repo_root = std::path::absolute(repo.as_path())
        .unwrap_or_else(|_| repo.as_path().to_path_buf())
        .display()
        .to_string();
    let mut replacements =
        BTreeMap::from([("rev", rev.as_str().to_owned()), ("repo_root", repo_root)]);
    if let Some(path) = source_path {
        replacements.insert("path", path.to_owned());
    }
    let mut out = Vec::with_capacity(argv.len());
    for arg in argv {
        if arg.contains("{path}") && source_path.is_none() {
            return Err(JevError::config_invalid(
                "command argv contains {path}, but the input has no path field; set path = \"<repo-relative file>\" on the command input",
            ));
        }
        // One left-to-right pass: a path containing `{rev}`, or a rev
        // containing `{repo_root}`, is emitted verbatim (RBP observation).
        out.push(expand_placeholders(arg, &replacements));
    }
    Ok(out)
}

/// Producer name for an error response: the file stem of `argv[0]`.
fn producer_name(program: &str) -> String {
    Path::new(program)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or(program)
        .to_owned()
}

/// Error for a non-zero exit: the producer's own error response when its
/// last non-empty stderr line is one, else `EXTRACTION_FAILED` with the tail.
fn exit_error(program: &str, code: i32, stderr: &[u8]) -> JevError {
    let last_line = String::from_utf8_lossy(stderr)
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_owned);
    if let Some(line) = last_line
        && let Ok(response) = serde_json::from_str::<ScAgentErrorResponse>(&line)
    {
        return redact_error(JevError::from_error_response(
            &producer_name(program),
            &response,
        ));
    }
    let tail = stderr_tail(stderr);
    let detail = if tail.is_empty() {
        format!("exit {code}")
    } else {
        format!("exit {code}; stderr: {tail}")
    };
    redact_error(JevError::extraction_failed(detail))
}

fn run_argv(repo: &RepoRoot, argv: &[String], timeout: Duration) -> Result<Vec<u8>, JevError> {
    let mut command = Command::new(&argv[0]);
    apply_env_allowlist(&mut command);
    command
        .args(&argv[1..])
        .current_dir(repo.as_path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| {
        JevError::extraction_failed(format!(
            "failed to spawn {} (cause: {})",
            argv[0],
            error.kind()
        ))
    })?;
    let stdout = child
        .stdout
        .take()
        .expect("stdout is piped, so the child has a stdout handle");
    let stderr = child
        .stderr
        .take()
        .expect("stderr is piped, so the child has a stderr handle");
    let stdout_reader = PipeReader::spawn(move || read_capped(stdout, MAX_COMMAND_STDOUT_BYTES));
    let stderr_reader = PipeReader::spawn(move || read_tail(stderr, STDERR_TAIL_BYTES));

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() >= timeout => {
                let deadline = Instant::now() + CLEANUP_BUDGET;
                kill_and_reap(child, deadline);
                let _ = stdout_reader.finish_by(deadline);
                let _ = stderr_reader.finish_by(deadline);
                return Err(JevError::extraction_timeout(timeout.as_secs()));
            }
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(error) => {
                let deadline = Instant::now() + CLEANUP_BUDGET;
                kill_and_reap(child, deadline);
                let _ = stdout_reader.finish_by(deadline);
                let _ = stderr_reader.finish_by(deadline);
                return Err(JevError::extraction_failed(format!(
                    "failed to wait for command (cause: {})",
                    error.kind()
                )));
            }
        }
    };

    let deadline = Instant::now() + CLEANUP_BUDGET;
    let stdout = match stdout_reader.finish_by(deadline) {
        Ok(stdout) => stdout,
        Err(ReaderStop::Stopped) => {
            let _ = stderr_reader.finish_by(deadline);
            return Err(JevError::extraction_failed("command stdout reader stopped"));
        }
        Err(ReaderStop::TimedOut) => {
            let _ = stderr_reader.finish_by(deadline);
            return Err(JevError::extraction_failed(
                "command stdout reader did not finish before the cleanup deadline",
            ));
        }
    };
    let stderr = stderr_reader.finish_by(deadline).unwrap_or_default();
    if !status.success() {
        return Err(exit_error(&argv[0], status.code().unwrap_or(-1), &stderr));
    }
    match stdout {
        Ok(bytes) => Ok(bytes),
        Err(CapError::TooLarge) => Err(JevError::extraction_invalid(format!(
            "stdout exceeds {MAX_COMMAND_STDOUT_BYTES} bytes"
        ))),
        Err(CapError::Io(error)) => Err(JevError::extraction_failed(format!(
            "stdout read failed (cause: {})",
            error.kind()
        ))),
    }
}

enum CapError {
    TooLarge,
    Io(std::io::Error),
}

enum ReaderStop {
    TimedOut,
    Stopped,
}

/// Pipe drain that reports completion on a channel.
///
/// `finish_by` waits with `recv_timeout` and then detaches the worker. It
/// never calls `JoinHandle::join`, which has no deadline. A reader still
/// blocked when the deadline passes cannot keep the caller running; closing
/// the child (see [`kill_and_reap`]) is what unblocks `read`.
struct PipeReader<T> {
    rx: mpsc::Receiver<T>,
    handle: thread::JoinHandle<()>,
}

impl<T> PipeReader<T> {
    fn spawn(work: impl FnOnce() -> T + Send + 'static) -> Self
    where
        T: Send + 'static,
    {
        let (tx, rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let _ = tx.send(work());
        });
        Self { rx, handle }
    }

    fn finish_by(self, deadline: Instant) -> Result<T, ReaderStop> {
        let wait = deadline.saturating_duration_since(Instant::now());
        let received = self.rx.recv_timeout(wait);
        drop(self.handle);
        match received {
            Ok(value) => Ok(value),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(ReaderStop::TimedOut),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(ReaderStop::Stopped),
        }
    }
}

/// Kills `child` and reaps it until `deadline`.
///
/// `Child`'s destructor calls `wait` with no deadline. When the process is
/// still alive at `deadline`, or `try_wait` fails, the handle is left
/// undropped so this function can return. `Child::kill` is `SIGKILL` on Unix
/// and process termination on Windows.
fn kill_and_reap(child: Child, deadline: Instant) {
    let mut child = ManuallyDrop::new(child);
    let _ = child.kill();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                drop(ManuallyDrop::into_inner(child));
                return;
            }
            Ok(None) if Instant::now() >= deadline => return,
            Ok(None) => {
                let slice = POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now()));
                if slice.is_zero() {
                    return;
                }
                thread::sleep(slice);
            }
            Err(_) => return,
        }
    }
}

fn read_capped(mut pipe: impl Read, cap: usize) -> Result<Vec<u8>, CapError> {
    let mut buf = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let read = pipe.read(&mut chunk).map_err(CapError::Io)?;
        if read == 0 {
            return Ok(buf);
        }
        if buf.len().saturating_add(read) > cap {
            while pipe.read(&mut chunk).unwrap_or(0) > 0 {}
            return Err(CapError::TooLarge);
        }
        buf.extend_from_slice(&chunk[..read]);
    }
}

/// Drains `pipe` and keeps only its last `keep` bytes.
fn read_tail(mut pipe: impl Read, keep: usize) -> Vec<u8> {
    let mut tail = Vec::with_capacity(keep);
    let mut chunk = [0_u8; 8192];
    loop {
        let read = match pipe.read(&mut chunk) {
            Ok(0) | Err(_) => return tail,
            Ok(read) => read,
        };
        tail.extend_from_slice(&chunk[..read]);
        if tail.len() > keep {
            let excess = tail.len() - keep;
            tail.drain(..excess);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::test_support::{self, rel, temp_dir};

    const EMIT: &str = r#"
import json, sys
path, rev, root = sys.argv[1], sys.argv[2], sys.argv[3]
if not root:
    raise SystemExit("missing root")
doc = {
    "schema": "sc/subjects/1",
    "source": {"path": path, "rev": rev, "blob_sha": "b" * 40},
    "producer": {
        "name": "fixture",
        "version": "0.1.0",
        "language": "python",
        "grammar": "fixture",
        "grammar_version": "0",
        "query_sha256": "a" * 64,
        "mode": "matches",
    },
    "subjects": [{
        "id": f"{path}:1",
        "capture": "arm",
        "text": "arm",
        "span": {"start_line": 1, "end_line": 1, "start_byte": 0, "end_byte": 3},
    }],
}
sys.stdout.write(json.dumps(doc))
"#;

    /// Runs `argv` at `WORKTREE` with no `path` field.
    fn run(repo: &RepoRoot, argv: &[String], timeout_seconds: u64) -> Result<Extracted, JevError> {
        extract_command(repo, &Rev::worktree(), argv, timeout_seconds, None)
    }

    #[test]
    fn command_extractor_success_substitutes_placeholders() {
        let dir = temp_dir("emit");
        let script = dir.path().join("emit.py");
        std::fs::write(&script, EMIT).expect("script");
        std::fs::create_dir(dir.path().join("src")).expect("src");
        std::fs::write(dir.path().join("src/hook.py"), "x = 1\n").expect("hook");
        let repo = RepoRoot::new(dir.path());
        let argv: Vec<String> = vec![
            test_support::python().into(),
            script.display().to_string(),
            "{path}".into(),
            "{rev}".into(),
            "{repo_root}".into(),
        ];
        let extracted = extract_command(
            &repo,
            &Rev::worktree(),
            &argv,
            30,
            Some(&rel("src/hook.py")),
        )
        .expect("command");
        let document = extracted.document.expect("document");
        assert_eq!(document.source().path, "src/hook.py");
        assert_eq!(document.source().rev.as_str(), Rev::WORKTREE);
        assert_eq!(extracted.subjects[0].text(), "arm");
        assert_eq!(extracted.subjects[0].id(), "src/hook.py:1");
    }

    #[test]
    fn command_extractor_path_placeholder_requires_source() {
        let dir = temp_dir("nopath");
        let repo = RepoRoot::new(dir.path());
        let error = run(&repo, &["python3".into(), "{path}".into()], 30).expect_err("path");
        assert_eq!(error.code(), "CONFIG_INVALID");
        assert!(
            error.fix_hint().contains("path field"),
            "{}",
            error.fix_hint()
        );
    }

    #[test]
    fn command_extractor_nonzero_exit_is_extraction_failed() {
        let dir = temp_dir("exit");
        let repo = RepoRoot::new(dir.path());
        let input: Vec<String> = vec![
            test_support::python().into(),
            "-c".into(),
            "import sys; sys.exit(2)".into(),
        ];
        let error = run(&repo, &input, 30).expect_err("exit");
        assert_eq!(error.code(), "EXTRACTION_FAILED");
        assert!(error.to_string().contains("exit 2"));
    }

    #[test]
    fn command_extractor_timeout() {
        let dir = temp_dir("sleep");
        let repo = RepoRoot::new(dir.path());
        let input: Vec<String> = vec![
            test_support::python().into(),
            "-c".into(),
            "import time; time.sleep(30)".into(),
        ];
        let error = run(&repo, &input, 1).expect_err("timeout");
        assert_eq!(error.code(), "EXTRACTION_TIMEOUT");
        match error {
            JevError::ExtractionTimeout {
                timeout_seconds, ..
            } => assert_eq!(timeout_seconds, 1),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn command_extractor_invalid_stdout() {
        let dir = temp_dir("bad");
        let repo = RepoRoot::new(dir.path());
        let input: Vec<String> = vec![
            test_support::python().into(),
            "-c".into(),
            "print('nope')".into(),
        ];
        let error = run(&repo, &input, 30).expect_err("stdout");
        assert_eq!(error.code(), "EXTRACTION_INVALID");
    }

    #[test]
    fn command_extractor_empty_subjects() {
        let dir = temp_dir("empty");
        let repo = RepoRoot::new(dir.path());
        let code = r#"import json,sys; sys.stdout.write(json.dumps({"schema":"sc/subjects/1","source":{"path":"a.py","rev":"HEAD","blob_sha":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"},"producer":{"name":"fixture","version":"0","language":"python","grammar":"fixture","grammar_version":"0","query_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","mode":"matches"},"subjects":[]}))"#;
        let input: Vec<String> = vec![test_support::python().into(), "-c".into(), code.into()];
        let error = run(&repo, &input, 30).expect_err("empty");
        assert_eq!(error.code(), "EXTRACTION_EMPTY");
    }

    #[test]
    fn command_extractor_failure_carries_stderr_tail() {
        let dir = temp_dir("stderr");
        let repo = RepoRoot::new(dir.path());
        let input: Vec<String> = vec![
            test_support::python().into(),
            "-c".into(),
            "import sys; sys.stderr.write('x' * 9000 + 'boom-diagnostic'); sys.exit(3)".into(),
        ];
        let error = run(&repo, &input, 30).expect_err("exit");
        assert_eq!(error.code(), "EXTRACTION_FAILED");
        let text = error.to_string();
        assert!(text.contains("exit 3"), "{text}");
        assert!(text.contains("boom-diagnostic"), "{text}");
        assert!(text.len() < 5000, "stderr tail is bounded");
    }

    #[test]
    fn repo_root_placeholder_is_absolute() {
        let argv = substitute_placeholders(
            &["prog".to_owned(), "{repo_root}".to_owned()],
            &RepoRoot::new("relative-sub"),
            &Rev::worktree(),
            None,
        )
        .expect("argv");
        let root = std::path::Path::new(&argv[1]);
        assert!(root.is_absolute(), "{}", argv[1]);
        assert!(root.ends_with("relative-sub"));
    }

    /// A source path containing `{rev}` and a rev containing `{repo_root}`
    /// are substituted once and kept verbatim.
    #[test]
    fn placeholder_values_are_not_re_expanded() {
        let argv = substitute_placeholders(
            &[
                "prog".to_owned(),
                "{path}".to_owned(),
                "{rev}".to_owned(),
                "{unknown}".to_owned(),
            ],
            &RepoRoot::new("repo"),
            &Rev::new("v{repo_root}"),
            Some("dir/{rev}.py"),
        )
        .expect("argv");
        assert_eq!(argv[1], "dir/{rev}.py");
        assert_eq!(argv[2], "v{repo_root}");
        assert_eq!(argv[3], "{unknown}");
    }

    const PROBE_ENV: &str = "SC_JEV_CHECK_ENV_PROBE_API_KEY";
    const PROBE_SECRET_ENV: &str = "SC_JEV_CHECK_PROBE_SECRET";
    const PROBE_VALUE: &str = "sentinel-do-not-leak-7f2c";
    const PROBE_DONE: &str = "env-probe-ran";

    const PRINT_ENV: &str = r#"
import json, os, sys
text = "\n".join(f"{k}={v}" for k, v in sorted(os.environ.items()))
n = len(text.encode())
doc = {
    "schema": "sc/subjects/1",
    "source": {"path": "env.txt", "rev": "WORKTREE", "blob_sha": "b" * 40},
    "producer": {
        "name": "fixture", "version": "0", "language": "text", "grammar": "fixture",
        "grammar_version": "0", "query_sha256": "a" * 64, "mode": "matches",
    },
    "subjects": [{
        "id": "env.txt:1", "capture": "env", "text": text,
        "span": {"start_line": 1, "end_line": 1, "start_byte": 0, "end_byte": n},
    }],
}
sys.stdout.write(json.dumps(doc))
"#;

    /// Re-runs this test binary with a sentinel credential in its
    /// environment. `unsafe_code` is forbidden, so the test cannot set the
    /// variable in its own process.
    #[test]
    fn command_extractor_child_env_excludes_secrets() {
        let exe = std::env::current_exe().expect("test binary");
        let mut command = Command::new(exe);
        command
            .args([
                "--exact",
                "extract::command::tests::env_probe_child",
                "--include-ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(PROBE_ENV, PROBE_VALUE)
            .env(PROBE_SECRET_ENV, PROBE_VALUE);
        let output = crate::deadline::spawn_with_deadline(&mut command, None);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "probe failed: {stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(stdout.contains(PROBE_DONE), "probe did not run: {stdout}");
    }

    #[test]
    #[ignore = "child half of command_extractor_child_env_excludes_secrets"]
    fn env_probe_child() {
        if std::env::var_os(PROBE_ENV).is_none() {
            return;
        }
        let dir = temp_dir("env-probe");
        let repo = RepoRoot::new(dir.path());
        let input: Vec<String> = vec![test_support::python().into(), "-c".into(), PRINT_ENV.into()];
        let extracted = run(&repo, &input, 30).expect("env probe");
        let text = &extracted.subjects[0].text();
        assert!(!text.contains(PROBE_VALUE), "sentinel leaked: {text}");
        assert!(!text.contains(PROBE_ENV), "key name leaked: {text}");
        assert!(!text.contains(PROBE_SECRET_ENV), "SC_ name leaked: {text}");
        assert!(
            text.lines()
                .any(|line| line.to_ascii_uppercase().starts_with("PATH=")),
            "PATH is kept: {text}"
        );

        let echo: Vec<String> = vec![
            test_support::python().into(),
            "-c".into(),
            "import sys; sys.stderr.write(sys.argv[1]); sys.exit(1)".into(),
            PROBE_VALUE.into(),
        ];
        let error = run(&repo, &echo, 30).expect_err("echo");
        let rendered = format!("{error} {}", error.fix_hint());
        assert!(!rendered.contains(PROBE_VALUE), "{rendered}");
        assert!(rendered.contains("[redacted]"), "{rendered}");
        println!("{PROBE_DONE}");
    }
}
