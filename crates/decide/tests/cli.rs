//! The `decide` binary as a process: help, slug refusal, interrupts, broken stdout and the
//! fail-fast batch, against a loopback runner (no external network).
//!
//! Each child gets a cleared environment whose HOME is a tempdir: the `decide` binary has no
//! config-dir flag (decide reads `~/.sc-agent`, sc-observability's default for app
//! `sc-agent`), so HOME is the one way to give a child process its own config and log root.
//! On Unix `std::env::home_dir` reads HOME; on Windows it does not, which is one reason these
//! process tests are Unix-only (the others: Unix signals, `kill` and `mkfifo`). Only the
//! child's environment is set; the test process's own environment is never changed.

mod common;

/// AC: ANC-006 (the subcommands the Python parity test lists).
#[cfg(test)]
#[test]
fn help_lists_send_models_and_doctor() {
    let decide = env!("CARGO_BIN_EXE_decide");
    let output = common::run_bounded(std::process::Command::new(decide).arg("--help"));
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    for command in ["send", "models", "doctor"] {
        assert!(text.contains(command), "{text}");
    }
    let output = common::run_bounded(std::process::Command::new(decide).args(["send", "--help"]));
    assert!(String::from_utf8_lossy(&output.stdout).contains("--batch"));
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output, Stdio};
    use std::sync::mpsc;
    use std::time::Duration;

    use serde_json::Value;

    use std::sync::{Arc, Condvar, Mutex};

    use crate::common::{HANG_GUARD, NOUL, NOUL_RESPONSE, finish, run_bounded};

    const DECIDE: &str = env!("CARGO_BIN_EXE_decide");

    /// What the loopback daemon does with `POST /v1/systemone`.
    #[derive(Clone)]
    enum Mode {
        /// Accept and never answer (the connection is held open).
        Hang,
        /// Answer 200 with [`NOUL_RESPONSE`].
        Answer,
        /// Answer 200 once the test opens the gate (bounded by `HANG_GUARD`).
        AnswerAfter(Arc<(Mutex<bool>, Condvar)>),
    }

    /// A Mapika-style daemon on 127.0.0.1: `/health` says loaded; each systemone request is
    /// reported on the returned channel.
    fn loopback(mode: Mode) -> (u16, mpsc::Receiver<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut held = Vec::new();
            for stream in listener.incoming().flatten() {
                let sender = sender.clone();
                let stream_copy = stream.try_clone().expect("clone");
                held.push(stream_copy);
                let mode = mode.clone();
                std::thread::spawn(move || serve(stream, &mode, &sender));
            }
        });
        (port, receiver)
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
                        // Hold the connection: wait until the client goes away.
                        let mut rest = Vec::new();
                        let _ = reader.read_to_end(&mut rest);
                        return;
                    }
                    Mode::Answer => NOUL_RESPONSE,
                    Mode::AnswerAfter(gate) => {
                        let (lock, condvar) = &**gate;
                        let open = lock.lock().expect("lock");
                        let _ = condvar
                            .wait_timeout_while(open, HANG_GUARD, |open| !*open)
                            .expect("wait");
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

    /// A HOME whose `.sc-agent/default.json` names runner `dec` on `port`.
    fn home(port: u16) -> tempfile::TempDir {
        let home = tempfile::tempdir().expect("home");
        let config = home.path().join(".sc-agent");
        std::fs::create_dir_all(&config).expect("config dir");
        std::fs::write(
            config.join("default.json"),
            format!(
                r#"{{"local_runners":{{"dec":{{"kind":"mapika","host":"127.0.0.1","port":{port},"model":"jev-latest"}}}}}}"#
            ),
        )
        .expect("default.json");
        home
    }

    fn decide(home: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(DECIDE);
        command
            .args(args)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", home)
            .current_dir(env!("CARGO_MANIFEST_DIR"));
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
        find_rows(&home.join(".sc-agent"), &mut files);
        files
            .iter()
            .map(|file| std::fs::read_to_string(file).expect("rows").lines().count())
            .sum()
    }

    fn stderr_detail(output: &Output) -> Value {
        let text = String::from_utf8_lossy(&output.stderr);
        serde_json::from_str(text.lines().last().expect("one stderr line")).expect("stderr json")
    }

    /// AC: REQ-SAG-0004. `--project "Phase B"` is `decide.log` (cause `log.invalid_slug`);
    /// nothing is sent and no row is written.
    #[test]
    fn an_invalid_slug_is_refused_before_anything_is_sent() {
        let (port, sends) = loopback(Mode::Answer);
        let home = home(port);
        let file = batch_file(home.path(), 1);
        let output = run_bounded(&mut decide(
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
        assert_eq!(detail["id"], "decide.log");
        assert_eq!(detail["cause"]["id"], "log.invalid_slug");
        assert!(output.stdout.is_empty());
        assert!(sends.try_recv().is_err(), "nothing was sent");
        assert_eq!(rows(home.path()), 0);
    }

    /// AC: REQ-SAG-0504, end to end: a bad line stops decide before the runner is contacted.
    #[test]
    fn a_bad_batch_line_contacts_no_runner() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.set_nonblocking(true).expect("nonblocking");
        let port = listener.local_addr().expect("addr").port();
        let home = home(port);
        let path = home.path().join("bad.jsonl");
        let bad = r#"{"model":"jev-latest","state":"s","questions":{"q":{"type":"score"}}}"#;
        std::fs::write(&path, format!("{NOUL}\n{bad}\n{NOUL}\n")).expect("batch");
        let output = run_bounded(&mut decide(
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
        assert_eq!(detail["id"], "decide.request_invalid");
        assert!(
            detail["message"]
                .as_str()
                .is_some_and(|text| text.contains("line 2") && text.contains("criteria"))
        );
        assert!(
            listener.accept().is_err(),
            "no connection reached the runner"
        );
        assert!(output.stdout.is_empty());
    }

    /// AC: decide send, REQ-SAG-0701 output. A batch against the loopback runner prints one
    /// JSONL line per request with its ULID and position; exit 0.
    #[test]
    fn a_batch_prints_one_line_per_request() {
        let (port, _sends) = loopback(Mode::Answer);
        let home = home(port);
        let file = batch_file(home.path(), 3);
        let output = run_bounded(&mut decide(
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
        assert!(home.path().join(".sc-agent/logs/decide.log.jsonl").exists());
    }

    /// AC: REQ-SAG-0701 output. A closed stdout makes decide cancel and report `decide.write`.
    ///
    /// Ordered: the runner answers only after the test has closed decide's stdout (and seen
    /// the send arrive), so the first write always meets a closed pipe.
    #[test]
    fn a_closed_stdout_is_decide_write() {
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let (port, sends) = loopback(Mode::AnswerAfter(Arc::clone(&gate)));
        let home = home(port);
        let file = batch_file(home.path(), 3);
        let mut child = decide(
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
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
        drop(child.stdout.take());
        sends
            .recv_timeout(HANG_GUARD)
            .expect("a request reached the runner");
        {
            let (lock, condvar) = &*gate;
            *lock.lock().expect("lock") = true;
            condvar.notify_all();
        }
        let output = finish(child);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(stderr_detail(&output)["id"], "decide.write");
    }

    /// AC: REQ-SAG-0011 decide, NFR-SAG-0001. SIGINT and SIGTERM during a batch against a
    /// runner that never answers: every printed line is `decision.cancelled`, one session
    /// row per started request is on disk, exit 130 both times.
    #[test]
    fn an_interrupt_cancels_flushes_and_exits_130() {
        for signal in ["-INT", "-TERM"] {
            let (port, sends) = loopback(Mode::Hang);
            let home = home(port);
            let file = batch_file(home.path(), 3);
            let child = decide(
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
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn");
            for _ in 0..3 {
                sends
                    .recv_timeout(Duration::from_secs(20))
                    .expect("the batch is in flight");
            }
            let status = Command::new("kill")
                .args([signal, &child.id().to_string()])
                .status()
                .expect("kill");
            assert!(status.success());
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

    /// AC: REQ-SAG-0011 decide (RSH-001). SIGTERM while decide blocks reading a held-open
    /// stdin exits 130 at once, with nothing printed or sent.
    ///
    /// Synchronised without a sleep: `default.json` is a FIFO, so the test's open of its write
    /// end returns only once decide (past `signal::install`, the first thing `main` does) has
    /// opened it to load the configuration. Decide then blocks on stdin, which the test keeps
    /// open and never writes.
    #[test]
    fn sigterm_while_stdin_blocks_exits_130() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.set_nonblocking(true).expect("nonblocking");
        let port = listener.local_addr().expect("addr").port();
        let home = tempfile::tempdir().expect("home");
        let config = home.path().join(".sc-agent");
        std::fs::create_dir_all(&config).expect("config dir");
        let fifo = config.join("default.json");
        let made = Command::new("mkfifo").arg(&fifo).status().expect("mkfifo");
        assert!(made.success());
        let mut child = decide(
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
                "-",
            ],
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
        let held_stdin = child.stdin.take().expect("stdin");
        // Opening the FIFO's write end blocks until decide opens it to read; bounded.
        let (opened, writer) = mpsc::channel();
        {
            let fifo = fifo.clone();
            std::thread::spawn(move || {
                let _ = opened.send(std::fs::OpenOptions::new().write(true).open(&fifo));
            });
        }
        let mut writer = writer
            .recv_timeout(HANG_GUARD)
            .expect("decide opened its config")
            .expect("fifo");
        write!(
            writer,
            r#"{{"local_runners":{{"dec":{{"kind":"mapika","host":"127.0.0.1","port":{port},"model":"jev-latest"}}}}}}"#
        )
        .expect("config");
        drop(writer);
        let status = Command::new("kill")
            .args(["-TERM", &child.id().to_string()])
            .status()
            .expect("kill");
        assert!(status.success());
        let output = finish(child);
        drop(held_stdin);
        assert_eq!(
            output.status.code(),
            Some(130),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        assert!(listener.accept().is_err(), "nothing was sent");
    }
}
