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
