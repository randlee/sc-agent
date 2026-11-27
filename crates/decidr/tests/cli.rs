#[path = "../src/cli.rs"]
mod cli;
use clap::Parser;

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
            assert_eq!(timeout_secs, 10.0);
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
    let parsed =
        cli::Cli::try_parse_from(["decidr", "send", "--agent-role", "test", "--model", "laya"])?;
    match parsed.command {
        cli::Command::Send { timeout_secs, .. } => assert_eq!(timeout_secs, 120.0),
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
    let root = (0..1000)
        .map(|n| std::env::temp_dir().join(format!("sc-agent-j3-cli-{}-{n}", std::process::id())))
        .find(|path| std::fs::create_dir(path).is_ok())
        .ok_or("cannot create test directory")?;
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&root)
            .status()?
            .success()
    );
    assert!(
        Command::new("git")
            .args([
                "remote",
                "add",
                "origin",
                "https://github.com/randlee/sc-agent.git"
            ])
            .current_dir(&root)
            .status()?
            .success()
    );
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
    let mut process = Command::new(env!("CARGO_BIN_EXE_decidr"))
        .args([
            "--config-dir",
            root.to_str().ok_or("invalid temp path")?,
            "send",
            "--agent-role",
            "rust-developer",
            "--model",
            "laya",
        ])
        .env("DECIDR_LOG_ROOT", &root)
        .current_dir(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::from(std::fs::File::create(root.join("stdout"))?))
        .stderr(Stdio::from(std::fs::File::create(root.join("stderr"))?))
        .spawn()?;
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
