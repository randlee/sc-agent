#[path = "../src/cli.rs"]
mod cli;
use clap::Parser;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn isolated_command(command: &mut Command, root: &Path) {
    command
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("DECIDR_LOG_ROOT", root)
        .env("SC_AGENT_HOME", root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .current_dir(root);
    command.env(
        "GIT_CONFIG_GLOBAL",
        if cfg!(windows) { "NUL" } else { "/dev/null" },
    );
    #[cfg(windows)]
    if let Some(system_root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", system_root);
    }
}

fn fresh_temp_dir(prefix: &str) -> Result<PathBuf, std::io::Error> {
    (0..1000)
        .map(|n| std::env::temp_dir().join(format!("{prefix}-{}-{n}", std::process::id())))
        .find(|path| fs::create_dir(path).is_ok())
        .ok_or_else(|| std::io::Error::other("cannot create test directory"))
}

struct Cleanup(PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn req_sag_0004_0702_cli_accepts_project_role_timeout_and_batch_workers()
-> Result<(), Box<dyn std::error::Error>> {
    let parsed = cli::Cli::try_parse_from([
        "decidr",
        "--project",
        "phase-b",
        "send",
        "--agent-role",
        "rust-developer",
        "--timeout-secs",
        "10",
        "--batch",
        "--workers",
        "2",
    ])?;
    assert_eq!(parsed.project.as_deref(), Some("phase-b"));
    match parsed.command {
        cli::Command::Send {
            agent_role,
            timeout_secs,
            batch,
            workers,
            ..
        } => {
            assert_eq!(agent_role, "rust-developer");
            assert_eq!(timeout_secs, Some(10.0));
            assert!(batch);
            assert_eq!(workers, Some(2));
        }
        _ => return Err("send command not selected".into()),
    }
    Ok(())
}

#[test]
fn send_provider_flag_is_a_usage_error() {
    assert!(
        cli::Cli::try_parse_from([
            "decidr",
            "send",
            "--agent-role",
            "test",
            "--provider",
            "openai"
        ])
        .is_err()
    );
}
#[test]
fn local_timeout_default_is_120_seconds() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("SC_AGENT_DEFAULT_TIMEOUT_CHILD").is_none() {
        let root = fresh_temp_dir("sc-agent-decidr-default")?;
        let _cleanup = Cleanup(root.clone());
        let mut command = Command::new(std::env::current_exe()?);
        command.args([
            "--exact",
            "local_timeout_default_is_120_seconds",
            "--nocapture",
        ]);
        isolated_command(&mut command, &root);
        let output = command
            .env("SC_AGENT_DEFAULT_TIMEOUT_CHILD", "1")
            .output()?;
        assert!(
            output.status.success(),
            "child test failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("1 passed"),
            "child did not report one passing test:\n{}",
            String::from_utf8_lossy(&output.stdout)
        );
        return Ok(());
    }

    let config_dir = PathBuf::from(
        std::env::var_os("SC_AGENT_HOME").ok_or("SC_AGENT_HOME is required in child")?,
    );
    let parsed = cli::Cli::try_parse_from([
        "decidr",
        "--project",
        "phase-b",
        "send",
        "--agent-role",
        "test",
    ])?;
    match parsed.command {
        cli::Command::Send {
            model,
            timeout_secs,
            agent_role,
            ..
        } => {
            assert_eq!(model, None);
            assert_eq!(timeout_secs, None);
            let run = sc_agent::Run::new(
                sc_agent::RunArgs {
                    tool: "decidr".into(),
                    tool_version: env!("CARGO_PKG_VERSION").into(),
                    agent_role,
                    model,
                    timeout_secs,
                    project: Some("phase-b".into()),
                    config_dir: Some(config_dir),
                    ..Default::default()
                },
                None,
                std::sync::Arc::new(sc_agent::Cancel::default()),
            )?;
            assert_eq!(run.options.model, "jev");
            assert_eq!(run.options.timeout.as_secs_f64(), 120.0);
            run.close()?;
        }
        _ => return Err("send command not selected".into()),
    }
    Ok(())
}

#[test]
fn j3_decidr_row_identity() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let root = fresh_temp_dir("sc-agent-j3-cli")?;
    let _cleanup = Cleanup(root.clone());
    let mut init = Command::new("git");
    init.args(["init", "--quiet"]);
    isolated_command(&mut init, &root);
    assert!(init.status()?.success());
    let mut remote = Command::new("git");
    remote.args([
        "remote",
        "add",
        "origin",
        "https://github.com/randlee/sc-agent.git",
    ]);
    isolated_command(&mut remote, &root);
    assert!(remote.status()?.success());
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let port = address.port();
    std::fs::write(
        root.join("decidr.json"),
        serde_json::json!({"project":"phase-b","laya":{"host":"127.0.0.1","port":port}})
            .to_string(),
    )?;
    let server = std::thread::spawn(move || -> Result<(), String> {
        let (mut stream, _) = listener.accept().map_err(|e| e.to_string())?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(std::time::Duration::from_secs(10)))
            .map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
        let mut length = 0;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                return Err("request closed before headers".into());
            }
            if line == "\r\n" {
                break;
            }
            if let Some((name, value)) = line.split_once(':')
                && name.eq_ignore_ascii_case("content-length")
            {
                length = value.trim().parse::<usize>().map_err(|e| e.to_string())?;
            }
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body).map_err(|e| e.to_string())?;
        let response =
            serde_json::json!({"model":"local-model","answers":{"q":{"type":"noul","noul":0.5}}})
                .to_string();
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).map_err(|e| e.to_string())?;
        Ok(())
    });
    let mut process = Command::new(env!("CARGO_BIN_EXE_decidr"));
    process.args([
        "--config-dir",
        root.to_str().ok_or("invalid temp path")?,
        "send",
        "--agent-role",
        "rust-developer",
        "--model",
        "laya",
    ]);
    isolated_command(&mut process, &root);
    process
        .stdin(Stdio::piped())
        .stdout(Stdio::from(std::fs::File::create(root.join("stdout"))?))
        .stderr(Stdio::from(std::fs::File::create(root.join("stderr"))?));
    let mut process = process.spawn()?;
    process
        .stdin
        .take()
        .ok_or("stdin missing")?
        .write_all(br#"{"state":"evidence","questions":{"q":{"type":"noul"}}}"#)?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = process.try_wait()? {
            break Some(status);
        }
        if Instant::now() >= deadline {
            process.kill()?;
            process.wait()?;
            break None;
        }
        std::thread::park_timeout(Duration::from_millis(10));
    };
    let _ = TcpStream::connect_timeout(&address, Duration::from_secs(1));
    let server_result = server.join().map_err(|_| "server panicked")?;
    let status = status.ok_or("decidr did not exit within the hang guard")?;
    assert!(
        status.success(),
        "{}",
        std::fs::read_to_string(root.join("stderr"))?
    );
    server_result.map_err(std::io::Error::other)?;
    let path =
        root.join("sessions/github.com/randlee/sc-agent/rust-developer/phase-b/sessions.jsonl");
    let line = std::fs::read_to_string(path)?;
    let row: serde_json::Value = serde_json::from_str(line.trim())?;
    assert_eq!(row["fields"]["tool"], "decidr");
    assert_eq!(row["fields"]["tool_version"], env!("CARGO_PKG_VERSION"));
    Ok(())
}
