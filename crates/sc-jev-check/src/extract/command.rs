//! `command` extractor.
//!
//! Argv only: no shell. Placeholders `{path}`, `{rev}`, and `{repo_root}` are
//! replaced before spawn. The working directory is the repo root.

use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use sc_agent_types::{InputSpec, JevError, RepoRoot, Rev};

use super::Extracted;
use super::Extractor;
use super::private::Sealed;
use super::subjects::parse_subjects_bytes;

/// Largest stdout capture for one command.
///
/// Subjects documents are small. Stopping at 8 MiB bounds memory if a command
/// writes without exiting. Raising it does not change the payload byte cap.
const MAX_COMMAND_STDOUT_BYTES: usize = 8 * 1024 * 1024;

/// How often a running command is polled.
///
/// 20 ms keeps timeout tests short without a busy spin.
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Runs an argv array and parses stdout as `sc/subjects/1`.
#[derive(Debug, Clone, Copy)]
pub struct CommandExtractor;

impl Sealed for CommandExtractor {}

impl Extractor for CommandExtractor {
    fn kind(&self) -> &'static str {
        "command"
    }

    fn extract(
        &self,
        repo: &RepoRoot,
        rev: &Rev,
        input: &InputSpec,
    ) -> Result<Vec<sc_agent_types::Subject>, JevError> {
        Ok(self.extract_with_source(repo, rev, input, None)?.subjects)
    }
}

impl CommandExtractor {
    /// Runs `input`, substituting `source_path` for `{path}` when it is set.
    ///
    /// `{rev}` and `{repo_root}` always come from `rev` and `repo`. `{path}`
    /// without `source_path` is [`JevError::ConfigInvalid`] because a command
    /// input has no path field of its own.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::ExtractionFailed`] on a non-zero exit,
    /// [`JevError::ExtractionTimeout`] when `timeout_seconds` elapses,
    /// [`JevError::ExtractionInvalid`] when stdout is not `sc/subjects/1`,
    /// and [`JevError::ExtractionEmpty`] when the document has no subjects.
    pub fn extract_with_source(
        &self,
        repo: &RepoRoot,
        rev: &Rev,
        input: &InputSpec,
        source_path: Option<&str>,
    ) -> Result<Extracted, JevError> {
        let InputSpec::Command {
            command,
            timeout_seconds,
        } = input
        else {
            return Err(JevError::ConfigInvalid {
                detail: "command extractor received a different input kind".into(),
            });
        };
        if *timeout_seconds == 0 {
            return Err(JevError::ConfigInvalid {
                detail: "command timeout_seconds must be greater than zero".into(),
            });
        }
        let argv = substitute_placeholders(command, repo, rev, source_path)?;
        let stdout = run_argv(repo, &argv, Duration::from_secs(*timeout_seconds))?;
        parse_subjects_bytes(&stdout)
    }
}

pub(crate) fn substitute_placeholders(
    argv: &[String],
    repo: &RepoRoot,
    rev: &Rev,
    source_path: Option<&str>,
) -> Result<Vec<String>, JevError> {
    if argv.is_empty() || argv[0].is_empty() {
        return Err(JevError::ConfigInvalid {
            detail: "command argv must start with a program".into(),
        });
    }
    let repo_root = repo.as_path().display().to_string();
    let mut out = Vec::with_capacity(argv.len());
    for arg in argv {
        if arg.contains("{path}") && source_path.is_none() {
            return Err(JevError::ConfigInvalid {
                detail: "command argv contains {path} but no source path was provided".into(),
            });
        }
        let mut replaced = arg.clone();
        if let Some(path) = source_path {
            replaced = replaced.replace("{path}", path);
        }
        replaced = replaced.replace("{rev}", rev.as_str());
        replaced = replaced.replace("{repo_root}", &repo_root);
        out.push(replaced);
    }
    Ok(out)
}

fn run_argv(repo: &RepoRoot, argv: &[String], timeout: Duration) -> Result<Vec<u8>, JevError> {
    let mut command = Command::new(&argv[0]);
    command
        .args(&argv[1..])
        .current_dir(repo.as_path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| JevError::ExtractionFailed {
            detail: format!("failed to spawn {}: {error}", argv[0]),
        })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| JevError::ExtractionFailed {
            detail: "command stdout was not captured".into(),
        })?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| JevError::ExtractionFailed {
            detail: "command stderr was not captured".into(),
        })?;
    let stdout_thread = thread::spawn(move || read_capped(stdout, MAX_COMMAND_STDOUT_BYTES));
    let stderr_thread = thread::spawn(move || {
        let mut sink = Vec::new();
        let mut pipe = stderr;
        let _ = read_capped_discard(&mut pipe, &mut sink);
    });

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_thread.join();
                let _ = stderr_thread.join();
                return Err(JevError::ExtractionTimeout {
                    timeout_seconds: timeout.as_secs(),
                });
            }
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(error) => {
                return Err(JevError::ExtractionFailed {
                    detail: format!("failed to wait for command: {error}"),
                });
            }
        }
    };

    let stdout = stdout_thread
        .join()
        .map_err(|_| JevError::ExtractionFailed {
            detail: "command stdout reader stopped".into(),
        })?;
    let _ = stderr_thread.join();
    if !status.success() {
        let code = status.code().unwrap_or(-1);
        return Err(JevError::ExtractionFailed {
            detail: format!("exit {code}"),
        });
    }
    match stdout {
        Ok(bytes) => Ok(bytes),
        Err(CapError::TooLarge) => Err(JevError::ExtractionInvalid {
            detail: format!("stdout exceeds {MAX_COMMAND_STDOUT_BYTES} bytes"),
        }),
        Err(CapError::Io(error)) => Err(JevError::ExtractionFailed {
            detail: format!("stdout read failed: {error}"),
        }),
    }
}

enum CapError {
    TooLarge,
    Io(std::io::Error),
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

fn read_capped_discard(pipe: &mut impl Read, sink: &mut Vec<u8>) -> std::io::Result<()> {
    let mut chunk = [0_u8; 8192];
    loop {
        let read = pipe.read(&mut chunk)?;
        if read == 0 {
            return Ok(());
        }
        if sink.len() < 4096 {
            let keep = 4096usize.saturating_sub(sink.len()).min(read);
            sink.extend_from_slice(&chunk[..keep]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::test_support::{self, temp_dir};
    use sc_agent_types::InputSpec;

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

    fn command(argv: Vec<String>, timeout_seconds: u64) -> InputSpec {
        InputSpec::Command {
            command: argv,
            timeout_seconds,
        }
    }

    #[test]
    fn command_extractor_success_substitutes_placeholders() {
        let dir = temp_dir("emit");
        let script = dir.path().join("emit.py");
        std::fs::write(&script, EMIT).expect("script");
        let repo = RepoRoot::new(dir.path());
        let rev = Rev::new("abc123");
        let input = command(
            vec![
                test_support::python().into(),
                script.display().to_string(),
                "{path}".into(),
                "{rev}".into(),
                "{repo_root}".into(),
            ],
            30,
        );
        let extracted = CommandExtractor
            .extract_with_source(&repo, &rev, &input, Some("src/hook.py"))
            .expect("command");
        let document = extracted.document.expect("document");
        assert_eq!(document.source.path, "src/hook.py");
        assert_eq!(document.source.rev.as_str(), "abc123");
        assert_eq!(extracted.subjects[0].text, "arm");
        assert_eq!(extracted.subjects[0].id, "src/hook.py:1");
    }

    #[test]
    fn command_extractor_path_placeholder_requires_source() {
        let dir = temp_dir("nopath");
        let repo = RepoRoot::new(dir.path());
        let input = command(vec!["python3".into(), "{path}".into()], 30);
        let error = CommandExtractor
            .extract(&repo, &Rev::worktree(), &input)
            .expect_err("path");
        assert_eq!(error.code(), "CONFIG_INVALID");
    }

    #[test]
    fn command_extractor_nonzero_exit_is_extraction_failed() {
        let dir = temp_dir("exit");
        let repo = RepoRoot::new(dir.path());
        let input = command(
            vec![
                test_support::python().into(),
                "-c".into(),
                "import sys; sys.exit(2)".into(),
            ],
            30,
        );
        let error = CommandExtractor
            .extract(&repo, &Rev::worktree(), &input)
            .expect_err("exit");
        assert_eq!(error.code(), "EXTRACTION_FAILED");
        assert!(error.to_string().contains("exit 2"));
    }

    #[test]
    fn command_extractor_timeout() {
        let dir = temp_dir("sleep");
        let repo = RepoRoot::new(dir.path());
        let input = command(
            vec![
                test_support::python().into(),
                "-c".into(),
                "import time; time.sleep(30)".into(),
            ],
            1,
        );
        let error = CommandExtractor
            .extract(&repo, &Rev::worktree(), &input)
            .expect_err("timeout");
        assert_eq!(error.code(), "EXTRACTION_TIMEOUT");
        match error {
            JevError::ExtractionTimeout { timeout_seconds } => assert_eq!(timeout_seconds, 1),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn command_extractor_invalid_stdout() {
        let dir = temp_dir("bad");
        let repo = RepoRoot::new(dir.path());
        let input = command(
            vec![
                test_support::python().into(),
                "-c".into(),
                "print('nope')".into(),
            ],
            30,
        );
        let error = CommandExtractor
            .extract(&repo, &Rev::worktree(), &input)
            .expect_err("stdout");
        assert_eq!(error.code(), "EXTRACTION_INVALID");
    }

    #[test]
    fn command_extractor_empty_subjects() {
        let dir = temp_dir("empty");
        let repo = RepoRoot::new(dir.path());
        let code = r#"import json,sys; sys.stdout.write(json.dumps({"schema":"sc/subjects/1","source":{"path":"a.py","rev":"HEAD","blob_sha":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"},"producer":{"name":"fixture","version":"0","language":"python","grammar":"fixture","grammar_version":"0","query_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","mode":"matches"},"subjects":[]}))"#;
        let input = command(
            vec![test_support::python().into(), "-c".into(), code.into()],
            30,
        );
        let error = CommandExtractor
            .extract(&repo, &Rev::worktree(), &input)
            .expect_err("empty");
        assert_eq!(error.code(), "EXTRACTION_EMPTY");
    }
}
