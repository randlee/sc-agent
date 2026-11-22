use clap::{Parser, Subcommand};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
#[derive(Parser)]
struct Cli {
    #[arg(long, global = true)]
    config_dir: Option<PathBuf>,
    #[arg(long, global = true, help = "Project/phase for session records")]
    project: Option<String>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Models,
    Doctor,
    Send {
        #[arg(long, default_value = "jev")]
        model: String,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long, default_value = "decidr")]
        agent_role: String,
        #[arg(long)]
        input: Option<PathBuf>,
        #[arg(long)]
        batch: bool,
        #[arg(long)]
        workers: Option<usize>,
        #[arg(long, default_value = "120")]
        timeout_secs: f64,
    },
}
fn main() {
    let value = match sc_agent::default_logger("decidr") {
        Err(error) => sc_agent::result(Err(error)),
        Ok(logger) => {
            let outcome = (|| -> Result<Value, sc_agent::Error> {
                let cli = Cli::try_parse().map_err(|e| sc_agent::error("decidr.usage", e))?;
                if matches!(cli.command, Command::Models) {
                    return Ok(json!(sc_agent::PROVIDERS));
                }
                if matches!(cli.command, Command::Doctor) {
                    return sc_agent::doctor(cli.config_dir.as_deref(), cli.project.as_deref());
                }
                let (root, repo) = sc_agent::repository()?;
                let global = cli
                    .config_dir
                    .or_else(|| std::env::home_dir().map(|p| p.join(".sc-agent")))
                    .ok_or_else(|| sc_agent::error("config.home", "Home directory unavailable"))?;
                let local = root.join(".sc/sc-agent");
                let mut settings = sc_agent::config(
                    root.join(".git").exists().then_some(local.as_path()),
                    &global,
                    &std::env::vars_os().collect::<Vec<_>>(),
                    cli.project.as_deref(),
                )?
                .config;
                let Command::Send {
                    model,
                    provider,
                    agent_role,
                    input,
                    batch,
                    workers,
                    timeout_secs,
                } = cli.command
                else {
                    return Ok(Value::Null);
                };
                if let Some(workers) = workers {
                    settings.workers = workers;
                }
                let mut text = String::new();
                match input {
                    Some(path) => {
                        text = std::fs::read_to_string(path)
                            .map_err(|e| sc_agent::error("decidr.input_read", e))?
                    }
                    None => {
                        std::io::stdin()
                            .read_to_string(&mut text)
                            .map_err(|e| sc_agent::error("decidr.input_read", e))?;
                    }
                }
                let options = sc_agent::Options {
                    settings,
                    model,
                    provider,
                    agent_role,
                    timeout: Duration::try_from_secs_f64(timeout_secs)
                        .map_err(|e| sc_agent::error("decidr.invalid_request_timeout", e))?,
                    base_url: None,
                    root,
                    log_root: sc_agent::log_root("decidr")?,
                    repo,
                };
                let cancelled = Arc::new(AtomicBool::new(false));
                let flag = cancelled.clone();
                ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst))
                    .map_err(|e| sc_agent::error("decidr.signal", e))?;
                let pause = Mutex::new(());
                let result = if batch {
                    let requests = text
                        .lines()
                        .enumerate()
                        .map(|(i, line)| {
                            serde_json::from_str(line).map_err(|e| {
                                sc_agent::error(
                                    "decision.request_parse",
                                    format!("line {}: {e}", i + 1),
                                )
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    sc_agent::batch(
                        &requests,
                        &options,
                        &logger,
                        (&cancelled, &pause),
                        &std::thread::sleep,
                        &sc_agent::os_keyring,
                        &|_, _| {},
                    )
                } else {
                    let request = serde_json::from_str(&text)
                        .map_err(|e| sc_agent::error("decision.request_parse", e))?;
                    sc_agent::send(
                        &request,
                        &options,
                        &logger,
                        (&cancelled, &pause),
                        &std::thread::sleep,
                        &sc_agent::os_keyring,
                    )
                };
                if cancelled.load(Ordering::SeqCst) {
                    return Err(sc_agent::error("decision.cancelled", "interrupted"));
                }
                result
            })();
            let value = sc_agent::report(&logger, outcome);
            let _ = logger.shutdown();
            value
        }
    };
    let status = if value.get("ok") == Some(&Value::Bool(true)) {
        0
    } else {
        1
    };
    if writeln!(std::io::stdout().lock(), "{value}").is_err() {
        std::process::exit(1);
    }
    std::process::exit(status);
}
