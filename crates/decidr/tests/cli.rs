//! The `decidr` binary as a process: help, slug refusal, interrupts, broken stdout and the
//! fail-fast batch, against a loopback runner (no external network).
//!
//! Each child gets a cleared environment, an explicit temporary HOME/USERPROFILE and cwd.
//! Its `.sc-agent/decidr.json` controls the runner and log root, so tests never read the real home. The process tests here are Unix-only
//! because they use Unix signals, `kill` and `mkfifo`. The test process's own environment is
//! never changed.

mod common;

/// Keep only the executable search path and the runtime loader path in an isolated child.
fn isolate_environment(
    command: &mut std::process::Command,
    path: Option<std::ffi::OsString>,
    loader_path: Option<std::ffi::OsString>,
) {
    command.env_clear().env("PATH", path.unwrap_or_default());
    if let Some(loader_path) = loader_path {
        command.env("LD_LIBRARY_PATH", loader_path);
    }
}

#[test]
fn isolated_child_environment_keeps_only_explicit_paths() {
    let mut command = std::process::Command::new("unused");
    command.env("HOME", "must-be-cleared");
    isolate_environment(&mut command, Some("bin".into()), Some("lib".into()));
    assert_eq!(
        command.get_envs().collect::<Vec<_>>(),
        vec![
            (
                std::ffi::OsStr::new("LD_LIBRARY_PATH"),
                Some(std::ffi::OsStr::new("lib"))
            ),
            (
                std::ffi::OsStr::new("PATH"),
                Some(std::ffi::OsStr::new("bin"))
            ),
        ]
    );
    let mut no_loader = std::process::Command::new("unused");
    isolate_environment(&mut no_loader, None, None);
    assert_eq!(
        no_loader.get_envs().collect::<Vec<_>>(),
        vec![(std::ffi::OsStr::new("PATH"), Some(std::ffi::OsStr::new(""))),]
    );
}

/// AC: ANC-006 (the subcommands the Python parity test lists).
#[cfg(test)]
#[test]
fn help_lists_send_models_and_doctor() {
    let decidr = env!("CARGO_BIN_EXE_decidr");
    let output = common::run_bounded(std::process::Command::new(decidr).arg("--help"));
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    for command in ["send", "models", "doctor"] {
        assert!(text.contains(command), "{text}");
    }
    let output = common::run_bounded(std::process::Command::new(decidr).args(["send", "--help"]));
    assert!(String::from_utf8_lossy(&output.stdout).contains("--batch"));
}

/// Invalid key_env is rejected before credential lookup, without retaining the pasted key.
#[test]
fn rejected_key_env_is_absent_from_error_detail_stderr_and_log_lines() {
    use sc_agent_log::{GeneralLogger, Level, LogContext};
    use serde_json::{Map, Value};
    let key = "sk-pasted-credential-not-a-variable";
    let root = tempfile::tempdir().expect("root");
    let config = root.path().join(".sc-agent");
    std::fs::create_dir(&config).expect("config dir");
    std::fs::write(
        config.join("decidr.json"),
        serde_json::json!({
            "log_root": root.path(),
            "local_runners": {"dec": {"kind": "mapika", "host": "127.0.0.1", "port": 9100,
                "model": "jev-latest", "key_env": key}}
        })
        .to_string(),
    )
    .expect("config");
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_decidr"));
    isolate_environment(
        &mut command,
        std::env::var_os("PATH"),
        std::env::var_os("LD_LIBRARY_PATH"),
    );
    let output = common::run_bounded(
        command
            .arg("models")
            .env("HOME", root.path())
            .env("USERPROFILE", root.path())
            .env("SC_AGENT_TEST_TRAP_PLATFORM_KEYRING", "1")
            .current_dir(root.path()),
    );
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).expect("stderr UTF8");
    let detail: Value =
        serde_json::from_str(stderr.lines().last().expect("error line")).expect("typed detail");
    assert_eq!(detail["id"], "decidr.config");
    assert_eq!(detail["cause"]["id"], "config.layer");
    assert_eq!(detail["cause"]["cause"]["id"], "config.invalid");
    assert!(
        detail["cause"]["cause"]["message"]
            .as_str()
            .expect("message")
            .contains("local_runners.dec.key_env")
    );
    assert!(!stderr.contains(key));
    assert!(
        !serde_json::to_string(&detail)
            .expect("detail JSON")
            .contains(key)
    );
    assert!(output.stdout.is_empty());
    // Config fails before the CLI can open its logger. Also exercise the actual
    // general-log writer with the returned detail, as a caller logging it would.
    let log = GeneralLogger::open_default(Some(root.path()), "decidr").expect("general logger");
    let mut fields = Map::new();
    fields.insert("error".into(), detail.clone());
    log.write(
        Level::Error,
        "decidr",
        "decidr.config",
        LogContext::NONE,
        fields,
    )
    .expect("write error");
    log.close().expect("close and flush");
    let text =
        std::fs::read_to_string(root.path().join("logs").join("decidr.log.jsonl")).expect("log");
    assert!(!text.is_empty());
    for line in text.lines() {
        assert!(!line.contains(key));
        let row: Value = serde_json::from_str(line).expect("log row");
        assert_eq!(row["fields"]["error"], detail);
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpStream;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output, Stdio};
    use std::sync::{Mutex, mpsc};

    use sc_agent_testkit::{ChildGuard, Gate, Loopback, recv_within, serve_each};
    use serde_json::Value;

    use crate::common::{HANG_GUARD, NOUL, NOUL_RESPONSE, finish, run_bounded};

    const DECIDR: &str = env!("CARGO_BIN_EXE_decidr");

    /// What the loopback daemon does with `POST /v1/systemone`.
    #[derive(Clone)]
    enum Mode {
        /// Accept and never answer (the connection is held open).
        Hang,
        /// Answer 200 with [`NOUL_RESPONSE`].
        Answer,
        /// Answer 200 once the test opens the gate (bounded by `HANG_GUARD`).
        AnswerAfter(Gate),
    }

    /// A Mapika-style daemon on 127.0.0.1 (testkit `serve_each`: every read and write bounded,
    /// the port held until it stops): `/health` says loaded; each systemone request is
    /// reported on the returned channel.
    fn loopback(mode: Mode) -> (Loopback, mpsc::Receiver<()>) {
        let (sender, receiver) = mpsc::channel();
        let sender = Mutex::new(sender);
        let server = serve_each(move |stream| {
            let sender = sender.lock().expect("sender").clone();
            serve(stream, &mode, &sender);
        });
        (server, receiver)
    }

    fn serve(stream: TcpStream, mode: &Mode, sender: &mpsc::Sender<()>) {
        let mut reader = BufReader::new(stream.try_clone().expect("clone"));
        let mut writer = stream;
        loop {
            let mut request_line = String::new();
            if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
                return;
            }
            let mut length = 0_usize;
            loop {
                let mut header = String::new();
                if reader.read_line(&mut header).unwrap_or(0) == 0 {
                    return;
                }
                let header = header.trim_end();
                if header.is_empty() {
                    break;
                }
                if let Some((name, value)) = header.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    length = value.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0; length];
            if reader.read_exact(&mut body).is_err() {
                return;
            }
            let body = if request_line.contains("/health") {
                r#"{"ok":true,"model":"jev-latest"}"#
            } else {
                let _ = sender.send(());
                match mode {
                    Mode::Hang => {
                        // Hold the connection: wait until the client goes away (the read is
                        // bounded by the stream's hang-guard timeout).
                        let mut rest = Vec::new();
                        let _ = reader.read_to_end(&mut rest);
                        return;
                    }
                    Mode::Answer => NOUL_RESPONSE,
                    Mode::AnswerAfter(gate) => {
                        gate.wait_open();
                        NOUL_RESPONSE
                    }
                }
            };
            let reply = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
                body.len()
            );
            if writer.write_all(reply.as_bytes()).is_err() {
                return;
            }
        }
    }

    /// The `decidr.json` of these tests: runner `dec` on `port`, and `log_root` = `config`.
    fn config_json(config: &Path, port: u16) -> String {
        serde_json::json!({
            "log_root": config,
            "local_runners": {
                "dec": { "kind": "mapika", "host": "127.0.0.1", "port": port, "model": "jev-latest" }
            }
        })
        .to_string()
    }

    /// A tempdir whose `.sc-agent/decidr.json` (the child's `--config-dir`) names runner `dec`
    /// on `port` and sets `log_root` to `.sc-agent` itself.
    fn home(port: u16) -> tempfile::TempDir {
        let home = tempfile::tempdir().expect("home");
        let config = home.path().join(".sc-agent");
        std::fs::create_dir_all(&config).expect("config dir");
        std::fs::write(config.join("decidr.json"), config_json(&config, port))
            .expect("decidr.json");
        home
    }

    fn decidr(home: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(DECIDR);
        super::isolate_environment(
            &mut command,
            std::env::var_os("PATH"),
            std::env::var_os("LD_LIBRARY_PATH"),
        );
        command
            .stdin(Stdio::null())
            .args(args)
            .env("HOME", home)
            .env("USERPROFILE", home)
            .env("SC_AGENT_TEST_TRAP_PLATFORM_KEYRING", "1")
            .current_dir(home);
        command
    }

    fn batch_file(dir: &Path, lines: usize) -> PathBuf {
        let path = dir.join("batch.jsonl");
        let text: String = (0..lines).map(|_| format!("{NOUL}\n")).collect();
        std::fs::write(&path, text).expect("batch");
        path
    }

    fn find_rows(dir: &Path, found: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                find_rows(&path, found);
            } else if path
                .file_name()
                .is_some_and(|name| name == "sessions.jsonl")
            {
                found.push(path);
            }
        }
    }

    fn rows(home: &Path) -> usize {
        let mut files = Vec::new();
        find_rows(&home.join(".sc-agent").join("sessions"), &mut files);
        files
            .iter()
            .map(|file| std::fs::read_to_string(file).expect("rows").lines().count())
            .sum()
    }

    fn stderr_detail(output: &Output) -> Value {
        let text = String::from_utf8_lossy(&output.stderr);
        serde_json::from_str(text.lines().last().expect("one stderr line")).expect("stderr json")
    }

    /// AC: REQ-SAG-0004. `--project "Phase B"` is `decidr.log` (cause `log.invalid_slug`);
    /// nothing is sent and no row is written.
    #[test]
    fn an_invalid_slug_is_refused_before_anything_is_sent() {
        let (runner, sends) = loopback(Mode::Answer);
        let home = home(runner.port());
        let file = batch_file(home.path(), 1);
        let output = run_bounded(&mut decidr(
            home.path(),
            &[
                "send",
                "--model",
                "dec",
                "--project",
                "Phase B",
                "--agent-role",
                "role",
                file.to_str().expect("utf-8"),
            ],
        ));
        assert_eq!(output.status.code(), Some(1));
        let detail = stderr_detail(&output);
        assert_eq!(detail["id"], "decidr.config");
        assert_eq!(detail["cause"]["id"], "config.layer");
        assert_eq!(detail["cause"]["cause"]["id"], "config.invalid");
        assert!(
            detail["cause"]["message"]
                .as_str()
                .expect("message")
                .contains("--project")
        );
        assert!(output.stdout.is_empty());
        assert!(sends.try_recv().is_err(), "nothing was sent");
        assert_eq!(rows(home.path()), 0);
    }

    #[test]
    fn layered_project_and_port_override_precede_send_and_sessions_are_retained() {
        let (server, sends) = loopback(Mode::Answer);
        let home = home(server.port());
        let work = home.path().join("work");
        std::fs::create_dir(&work).expect("work");
        assert!(
            run_bounded(Command::new("git").args(["init", "--quiet"]).arg(&work))
                .status
                .success()
        );
        assert!(
            run_bounded(Command::new("git").arg("-C").arg(&work).args([
                "remote",
                "add",
                "origin",
                "https://github.com/randlee/sc-agent.git"
            ]))
            .status
            .success()
        );
        let config = home.path().join(".sc-agent/decidr.json");
        // The file has a wrong port and no log_root: the environment supplies the real port,
        // and the shared default-root function supplies <home>/.sc-agent.
        std::fs::write(&config, serde_json::json!({"local_runners":{"laya":{"kind":"mapika","host":"127.0.0.1","port":1,"model":"jev-latest"}}}).to_string()).expect("config");
        let file = batch_file(home.path(), 1);
        let request = [
            "send",
            "--model",
            "laya",
            "--agent-role",
            "role",
            file.to_str().expect("path"),
        ];
        let missing = run_bounded(decidr(home.path(), &request).current_dir(&work));
        assert_eq!(missing.status.code(), Some(1));
        let detail = stderr_detail(&missing);
        assert_eq!(detail["id"], "decidr.config");
        assert_eq!(detail["cause"]["id"], "config.project_required");
        assert!(sends.try_recv().is_err());
        assert_eq!(rows(home.path()), 0);
        let old = home
            .path()
            .join(".sc-agent/sessions/local/m5/work/role/old/sessions.jsonl");
        std::fs::create_dir_all(old.parent().expect("parent")).expect("old folder");
        std::fs::write(&old, "retained session\n").expect("old bytes");
        std::fs::File::open(&old)
            .expect("old file")
            .set_times(std::fs::FileTimes::new().set_modified(std::time::SystemTime::UNIX_EPOCH))
            .expect("old mtime");
        let before = (
            std::fs::read(&old).expect("bytes"),
            std::fs::metadata(&old)
                .expect("metadata")
                .modified()
                .expect("mtime"),
        );
        let sent = run_bounded(
            decidr(home.path(), &request)
                .args(["--project", "phase-flag"])
                .current_dir(&work)
                .env("SC_PROJECT", "phase-env")
                .env(
                    "DECIDR_LOCAL_RUNNERS__LAYA__PORT",
                    server.port().to_string(),
                ),
        );
        assert!(
            sent.status.success(),
            "{}",
            String::from_utf8_lossy(&sent.stderr)
        );
        recv_within(&sends, HANG_GUARD);
        let session = home
            .path()
            .join(".sc-agent/sessions/github.com/randlee/sc-agent/role/phase-flag/sessions.jsonl");
        let row: Value =
            serde_json::from_str(&std::fs::read_to_string(session).expect("session")).expect("row");
        assert_eq!(row["fields"]["project"], "phase-flag");
        assert_eq!(row["fields"]["repo"], "github.com/randlee/sc-agent");
        assert!(
            home.path()
                .join(".sc-agent/logs/decidr.log.jsonl")
                .is_file()
        );
        assert_eq!(
            before,
            (
                std::fs::read(&old).expect("bytes"),
                std::fs::metadata(&old)
                    .expect("metadata")
                    .modified()
                    .expect("mtime")
            )
        );
        let override_root = home.path().join("override");
        let sent = run_bounded(
            decidr(home.path(), &request)
                .current_dir(&work)
                .env("SC_PROJECT", "phase-env")
                .env(
                    "DECIDR_LOCAL_RUNNERS__LAYA__PORT",
                    server.port().to_string(),
                )
                .env("DECIDR_LOG_ROOT", &override_root),
        );
        assert!(
            sent.status.success(),
            "{}",
            String::from_utf8_lossy(&sent.stderr)
        );
        recv_within(&sends, HANG_GUARD);
        assert!(override_root.join("logs/decidr.log.jsonl").is_file());
        assert!(
            override_root
                .join("sessions/github.com/randlee/sc-agent/role/phase-env/sessions.jsonl")
                .is_file()
        );
        let mut general_sessions = Vec::new();
        find_rows(&override_root.join("logs"), &mut general_sessions);
        assert!(general_sessions.is_empty());
    }

    #[test]
    fn workers_without_batch_is_typed_and_does_not_send() {
        let runner = serve_each(|_| {});
        let home = home(runner.port());
        let file = batch_file(home.path(), 1);
        let output = run_bounded(&mut decidr(
            home.path(),
            &[
                "send",
                "--model",
                "dec",
                "--project",
                "proj",
                "--agent-role",
                "role",
                "--workers",
                "2",
                file.to_str().expect("utf-8"),
            ],
        ));
        assert_eq!(output.status.code(), Some(1));
        let detail = stderr_detail(&output);
        assert_eq!(detail["id"], "decidr.invalid_workers");
        let message = detail["message"].as_str().expect("message");
        assert!(message.contains("2") && message.contains("--batch"));
        assert!(output.stdout.is_empty());
        assert_eq!(runner.stop(), 0, "no connection reached the runner");
    }

    /// AC: REQ-SAG-0504, end to end: a bad line stops decidr before the runner is contacted.
    #[test]
    fn a_bad_batch_line_contacts_no_runner() {
        let runner = serve_each(|_| {});
        let home = home(runner.port());
        let path = home.path().join("bad.jsonl");
        let bad = r#"{"model":"jev-latest","state":"s","questions":{"q":{"type":"score"}}}"#;
        std::fs::write(&path, format!("{NOUL}\n{bad}\n{NOUL}\n")).expect("batch");
        let output = run_bounded(&mut decidr(
            home.path(),
            &[
                "send",
                "--batch",
                "--model",
                "dec",
                "--project",
                "proj",
                "--agent-role",
                "role",
                path.to_str().expect("utf-8"),
            ],
        ));
        assert_eq!(output.status.code(), Some(1));
        let detail = stderr_detail(&output);
        assert_eq!(detail["id"], "decidr.request_invalid");
        assert!(
            detail["message"]
                .as_str()
                .is_some_and(|text| text.contains("line 2") && text.contains("criteria"))
        );
        assert_eq!(runner.stop(), 0, "no connection reached the runner");
        assert!(output.stdout.is_empty());
    }

    /// AC: decidr send, REQ-SAG-0701 output. A batch against the loopback runner prints one
    /// JSONL line per request with its ULID and position; exit 0.
    #[test]
    fn a_batch_prints_one_line_per_request() {
        let (runner, _sends) = loopback(Mode::Answer);
        let home = home(runner.port());
        let file = batch_file(home.path(), 3);
        let output = run_bounded(&mut decidr(
            home.path(),
            &[
                "send",
                "--batch",
                "--model",
                "dec",
                "--project",
                "proj",
                "--agent-role",
                "role",
                file.to_str().expect("utf-8"),
            ],
        ));
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let lines: Vec<Value> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|line| serde_json::from_str(line).expect("json"))
            .collect();
        assert_eq!(lines.len(), 3);
        let mut positions: Vec<u64> = lines
            .iter()
            .map(|line| line["position"].as_u64().expect("position"))
            .collect();
        positions.sort_unstable();
        assert_eq!(positions, [0, 1, 2]);
        assert!(
            lines
                .iter()
                .all(|line| line["session_id"].as_str().is_some_and(|id| id.len() == 26))
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("0.123456789"));
        assert_eq!(rows(home.path()), 3);
        assert!(
            home.path()
                .join(".sc-agent")
                .join("logs")
                .join("decidr.log.jsonl")
                .exists()
        );
    }

    /// AC: REQ-SAG-0701 output. A closed stdout makes decidr cancel and report `decidr.write`.
    ///
    /// Stdout's own socket has writes shut down before spawn, so inherited descriptors
    /// cannot make it writable. The runner still answers only after the send arrives.
    #[test]
    fn a_closed_stdout_is_decide_write() {
        use std::os::fd::OwnedFd;
        use std::os::unix::net::UnixStream;

        let (_reader, writer) = UnixStream::pair().expect("stdout socket pair");
        writer
            .shutdown(std::net::Shutdown::Write)
            .expect("shut down stdout writes");
        let gate = Gate::new();
        let opener = gate.opener();
        let (runner, sends) = loopback(Mode::AnswerAfter(gate));
        let home = home(runner.port());
        let file = batch_file(home.path(), 3);
        let child = ChildGuard::spawn(
            decidr(
                home.path(),
                &[
                    "send",
                    "--batch",
                    "--model",
                    "dec",
                    "--project",
                    "proj",
                    "--agent-role",
                    "role",
                    file.to_str().expect("utf-8"),
                ],
            )
            .stdout(Stdio::from(OwnedFd::from(writer)))
            .stderr(Stdio::piped()),
        );
        recv_within(&sends, HANG_GUARD);
        opener.open();
        let output = finish(child);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(stderr_detail(&output)["id"], "decidr.write");
    }

    /// AC: REQ-SAG-0011 decidr, NFR-SAG-0001. SIGINT and SIGTERM during a batch against a
    /// runner that never answers: every printed line is `decision.cancelled`, one session
    /// row per started request is on disk, exit 130 both times.
    #[test]
    fn an_interrupt_cancels_flushes_and_exits_130() {
        for signal in ["-INT", "-TERM"] {
            let (runner, sends) = loopback(Mode::Hang);
            let home = home(runner.port());
            let file = batch_file(home.path(), 3);
            let child = ChildGuard::spawn(
                decidr(
                    home.path(),
                    &[
                        "send",
                        "--batch",
                        "--model",
                        "dec",
                        "--project",
                        "proj",
                        "--agent-role",
                        "role",
                        file.to_str().expect("utf-8"),
                    ],
                )
                .stdout(Stdio::piped())
                .stderr(Stdio::piped()),
            );
            for _ in 0..3 {
                recv_within(&sends, HANG_GUARD);
            }
            let killed = run_bounded(Command::new("kill").args([signal, &child.id().to_string()]));
            assert!(
                killed.status.success(),
                "{}",
                String::from_utf8_lossy(&killed.stderr)
            );
            let output = finish(child);
            assert_eq!(
                output.status.code(),
                Some(130),
                "{signal}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let lines: Vec<Value> = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|line| serde_json::from_str(line).expect("json"))
                .collect();
            assert_eq!(lines.len(), 3, "{signal}");
            assert!(
                lines
                    .iter()
                    .all(|line| line["error"]["id"] == "decision.cancelled"),
                "{signal}: {lines:?}"
            );
            assert_eq!(
                rows(home.path()),
                3,
                "{signal}: one row per started request"
            );
        }
    }

    /// AC: REQ-SAG-0011 decidr (RSH-001). SIGTERM while decidr blocks reading its input exits
    /// 130 at once, with nothing printed or sent.
    ///
    /// Synchronised without a sleep: the batch input is a FIFO, so the test's open of its write
    /// end returns only once decidr (past `signal::install`, the first thing `main` does, and
    /// past loading its configuration) has opened it to read. Decide then blocks reading it;
    /// the test keeps it open and never writes.
    #[test]
    fn sigterm_while_the_input_blocks_exits_130() {
        let runner = serve_each(|_| {});
        let home = home(runner.port());
        let fifo = home.path().join("input.jsonl");
        let made = run_bounded(Command::new("mkfifo").arg(&fifo));
        assert!(
            made.status.success(),
            "{}",
            String::from_utf8_lossy(&made.stderr)
        );
        let child = ChildGuard::spawn(
            decidr(
                home.path(),
                &[
                    "send",
                    "--batch",
                    "--model",
                    "dec",
                    "--project",
                    "proj",
                    "--agent-role",
                    "role",
                    fifo.to_str().expect("utf-8"),
                ],
            )
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
        );
        // Opening the FIFO's write end blocks until decidr opens it to read; bounded.
        let (opened, writer) = mpsc::channel();
        {
            let fifo = fifo.clone();
            std::thread::spawn(move || {
                let _ = opened.send(std::fs::OpenOptions::new().write(true).open(&fifo));
            });
        }
        let held = recv_within(&writer, HANG_GUARD).expect("decidr opened its input");
        let killed = run_bounded(Command::new("kill").args(["-TERM", &child.id().to_string()]));
        assert!(
            killed.status.success(),
            "{}",
            String::from_utf8_lossy(&killed.stderr)
        );
        let output = finish(child);
        drop(held);
        assert_eq!(
            output.status.code(),
            Some(130),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        assert_eq!(runner.stop(), 0, "nothing was sent");
    }

    /// RSH-002: a `decidr.json` that is a FIFO is refused from its metadata
    /// (`config.not_a_file`), never opened, so decidr cannot block on it: it exits 1 at once.
    #[test]
    fn a_fifo_config_file_is_refused_without_blocking() {
        let home = tempfile::tempdir().expect("home");
        let config = home.path().join(".sc-agent");
        std::fs::create_dir_all(&config).expect("config dir");
        let made = run_bounded(Command::new("mkfifo").arg(config.join("decidr.json")));
        assert!(
            made.status.success(),
            "{}",
            String::from_utf8_lossy(&made.stderr)
        );
        let output = run_bounded(&mut decidr(home.path(), &["models"]));
        assert_eq!(output.status.code(), Some(1));
        let detail = stderr_detail(&output);
        assert_eq!(detail["id"], "decidr.config");
        assert_eq!(detail["cause"]["id"], "config.not_a_file");
    }
}

/// Doctor's binary path prints the shared loader rows even when the guarded keyring refuses.
#[test]
fn layered_doctor_reports_sources_files_and_keyring_without_writes() {
    use serde_json::{Value, json};
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    fn files(root: &Path) -> BTreeMap<PathBuf, (Vec<u8>, std::time::SystemTime)> {
        let mut snapshot = BTreeMap::new();
        for entry in std::fs::read_dir(root).expect("entries") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                snapshot.extend(files(&path));
            } else {
                snapshot.insert(
                    path.clone(),
                    (
                        std::fs::read(&path).expect("bytes"),
                        std::fs::metadata(path)
                            .expect("metadata")
                            .modified()
                            .expect("mtime"),
                    ),
                );
            }
        }
        snapshot
    }
    let (request_sent, requests) = std::sync::mpsc::channel();
    let server =
        sc_agent_testkit::serve_each(move |mut stream| {
            request_sent.send(()).expect("record unexpected request");
            let _ = std::io::Write::write_all(&mut stream,
            b"HTTP/1.1 500 Unexpected Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        });
    let root = tempfile::tempdir().expect("root");
    let home = root.path().join("home");
    // Exercise JSON escaping on Unix as well as Windows.
    let work = root.path().join(r"work\escaped");
    std::fs::create_dir_all(home.join(".sc-agent")).expect("home");
    std::fs::create_dir_all(work.join(".sc/sc-agent")).expect("work");
    let git = common::run_bounded(
        std::process::Command::new("git")
            .args(["init", "--quiet"])
            .arg(&work),
    );
    assert!(git.status.success());
    let work = sc_agent_log::Repo::work_tree_root(&work)
        .expect("worktree lookup")
        .expect("git worktree");
    let global = home.join(".sc-agent/sc-agent.json");
    let local = work.join(".sc").join("sc-agent").join("decidr.json");
    std::fs::write(&global, json!({"project":"global"}).to_string()).expect("global");
    std::fs::write(home.join(".sc-agent").join("decidr.json"), json!({
        "local_runners":{"laya":{"kind":"mapika","host":"127.0.0.1","port":server.port(),"model":"jev-latest"}}
    }).to_string()).expect("runner fixture");
    std::fs::write(&local, json!({"project":"phase-a","workers":3}).to_string()).expect("local");
    let command = || {
        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_decidr"));
        isolate_environment(
            &mut cmd,
            std::env::var_os("PATH"),
            std::env::var_os("LD_LIBRARY_PATH"),
        );
        cmd.arg("doctor")
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("SC_AGENT_TEST_TRAP_PLATFORM_KEYRING", "1")
            .current_dir(&work);
        cmd
    };
    let before = files(root.path());
    let output = common::run_bounded(&mut command());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).expect("doctor JSON");
    let loaded = sc_agent_config::load_layered(&sc_agent_config::LoadOptions {
        cli: sc_agent_config::CliName::Decidr,
        home: home.clone(),
        worktree: Some(work.clone()),
        default_log_root: sc_agent_log::default_log_root(&home),
        flags: BTreeMap::new(),
        flag_names: BTreeMap::new(),
        environment: Vec::new(),
    })
    .expect("canonical config");
    assert_eq!(
        report["settings"],
        serde_json::to_value(sc_agent_config::doctor_rows(&loaded)).expect("canonical rows")
    );
    assert_eq!(
        report["files"],
        serde_json::to_value(&loaded.files).expect("canonical files")
    );
    assert_eq!(report["files"].as_array().expect("files").len(), 4);
    assert_eq!(
        report["keyring"].as_array().expect("keyring rows").len(),
        sc_agent_types::Provider::ALL.len()
    );
    assert!(
        report["keyring"]
            .as_array()
            .expect("rows")
            .iter()
            .all(|row| row["error"]["id"] == "secrets.keyring_construction_refused")
    );
    assert_eq!(files(root.path()), before, "doctor changed a file");
    let outside = root.path().join("outside");
    std::fs::create_dir(&outside).expect("outside");
    let output = common::run_bounded(command().current_dir(&outside));
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).expect("outside JSON");
    assert_eq!(report["files"].as_array().expect("files").len(), 2);
    let output = common::run_bounded(
        command()
            .env("DECIDR_WORKERS", "3")
            .env("SC_PROJECT", "phase-d")
            .env("DECIDR_LOG_ROOT", root.path().join("other")),
    );
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).expect("env JSON");
    let rows = report["settings"].as_array().expect("rows");
    for (name, value, source) in [
        ("project", "phase-d", "SC_PROJECT"),
        ("workers", "3", "DECIDR_WORKERS"),
    ] {
        assert!(
            rows.iter().any(|row| row["setting"] == name
                && row["value"] == value
                && row["source"] == source)
        );
    }
    assert!(
        rows.iter()
            .any(|row| row["setting"] == "log_root" && row["source"] == "DECIDR_LOG_ROOT")
    );
    let output = common::run_bounded(command().env("DECIDR_WORKERS", "0"));
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("DECIDR_WORKERS"));
    std::fs::write(&local, "{").expect("malformed");
    let output = common::run_bounded(&mut command());
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr UTF8");
    let detail: sc_agent_types::ErrorDetail =
        serde_json::from_str(stderr.lines().last().expect("typed error line"))
            .expect("typed error JSON");
    assert_eq!(detail.id, "decidr.config");
    let cause = detail.cause.as_deref().expect("configuration cause");
    assert_eq!(cause.id, "config.json");
    assert!(
        cause
            .message
            .contains(local.to_str().expect("full config path"))
    );
    assert!(
        requests.try_recv().is_err(),
        "doctor sent a request to the configured runner"
    );
    assert!(
        !root.path().join("other").exists(),
        "doctor opened the configured log root"
    );
}
