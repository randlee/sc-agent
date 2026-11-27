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
