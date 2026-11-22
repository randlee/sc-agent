use clap::{Parser, Subcommand};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
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
        workers: Option<i64>,
        #[arg(long, default_value = "120")]
        timeout_secs: f64,
    },
}
fn run() -> Result<Value, sc_agent::Error> {
    let cli = Cli::try_parse().map_err(|e| sc_agent::error("decidr.usage", e))?;
    if matches!(cli.command, Command::Models) {
        return Ok(json!(sc_agent::PROVIDERS));
    }
    if matches!(cli.command, Command::Doctor) {
        return sc_agent::doctor("decidr", cli.config_dir.as_deref(), cli.project.as_deref());
    }
    let (root, repo) = sc_agent::repository()?;
    let global = cli
        .config_dir
        .map(Ok)
        .unwrap_or_else(|| sc_agent::home().map(|p| p.join(".sc-agent")))?;
    let local = root.join(".sc/sc-agent");
    let config = sc_agent::config(
        "decidr",
        root.join(".git").exists().then_some(local.as_path()),
        &global,
        &std::env::vars_os().collect::<Vec<_>>(),
        cli.project.as_deref(),
    )?;
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
    let (timeout, workers) = sc_agent::limits(timeout_secs, workers, batch)?;
    let mut text = String::new();
    match input {
        Some(path) => {
            text = std::fs::read_to_string(path)
                .map_err(|e| sc_agent::error("decidr.input_read", e))?
        }
        None => {
            std::io::stdin()
                .take(256 * 1024 * 1024 + 1)
                .read_to_string(&mut text)
                .map_err(|e| sc_agent::error("decidr.input_read", e))?;
        }
    }
    let mut settings = config.config;
    settings.workers = workers.unwrap_or(settings.workers);
    let options = sc_agent::Options {
        settings,
        model,
        provider,
        agent_role,
        timeout,
        base_url: None,
        root,
        log_root: sc_agent::log_root("decidr")?,
        repo,
        session_log: None,
    };
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();
    ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst))
        .map_err(|e| sc_agent::error("decidr.signal", e))?;
    let logger = sc_agent::default_logger("decidr")?;
    let pause = Mutex::new(());
    let sleep = |delay| {
        let (_sender, receiver) = std::sync::mpsc::channel::<()>();
        let _ = receiver.recv_timeout(delay);
    };
    let got = if batch {
        let requests = text
            .lines()
            .enumerate()
            .map(|(i, line)| {
                serde_json::from_str(line).map_err(|e| {
                    sc_agent::error("decision.request_parse", format!("line {}: {e}", i + 1))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        sc_agent::batch(
            &requests,
            &options,
            &logger,
            (&cancelled, &pause),
            &sleep,
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
            &sleep,
            &sc_agent::os_keyring,
        )
    };
    let _ = logger.shutdown();
    if cancelled.load(Ordering::SeqCst) {
        return Err(sc_agent::error("decision.cancelled", "interrupted"));
    }
    got
}
fn main() {
    let value = sc_agent::result(run());
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
