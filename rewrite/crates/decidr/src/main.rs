use clap::{Parser, Subcommand};
use serde_json::json;
use std::{fs, process::ExitCode};

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Models,
    Doctor {
        #[arg(long)]
        project: Option<String>,
    },
    Send {
        #[arg(long)]
        model: String,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        agent_role: String,
        request: String,
    },
}

fn main() -> ExitCode {
    let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = std::sync::Arc::clone(&cancelled);
    if let Err(e) =
        ctrlc::set_handler(move || flag.store(true, std::sync::atomic::Ordering::Relaxed))
    {
        eprintln!("Could not install Ctrl-C handler: {e}");
        return ExitCode::FAILURE;
    }
    let args = Args::parse();
    let action = match &args.command {
        Command::Models => "decidr.models",
        Command::Doctor { .. } => "decidr.doctor",
        Command::Send { .. } => "decidr.send",
    };
    let log = match sc_agent::open_default_logger("decidr", "DECIDR_LOG_ROOT") {
        Ok(log) => log,
        Err(e) => {
            println!("{e}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = sc_agent::log(&log, action) {
        println!("{e}");
        return ExitCode::FAILURE;
    }
    let result = match args.command {
        Command::Models => json!({"ok": true, "value": sc_agent::models()}),
        Command::Doctor { project } => sc_agent::doctor(project.as_deref()),
        Command::Send {
            model,
            provider,
            project,
            agent_role,
            request,
        } => match fs::read_to_string(request) {
            Ok(input) => sc_agent::send(
                &input,
                &model,
                provider.as_deref(),
                project.as_deref(),
                Some(&agent_role),
                &std::thread::sleep,
                Some(&cancelled),
            ),
            Err(_) => sc_agent::error(
                "invalid_request",
                "Cannot read request file",
                "Provide a readable request file",
            ),
        },
    };
    println!("{result}");
    if result.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
