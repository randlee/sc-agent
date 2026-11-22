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
fn run() -> Result<Value, sc_agent::Error> {
    let cli = Cli::try_parse().map_err(|e| sc_agent::error("decidr.usage", e))?;
    if matches!(cli.command, Command::Models) {
        return Ok(json!(sc_agent::PROVIDERS));
    }
    let (root, repo) = sc_agent::repository()?;
    let global = cli
        .config_dir
        .map(Ok)
        .unwrap_or_else(|| sc_agent::home().map(|p| p.join(".sc-agent")))?;
    let local = root.join(".sc/sc-agent");
    let config = sc_agent::config(
        Some(&local),
        &global,
        &std::env::vars_os().collect::<Vec<_>>(),
        cli.project.as_deref(),
    )?;
    if matches!(cli.command, Command::Doctor) {
        return Ok(
            json!({"settings":sc_config::layers::resolved(&config,sc_agent::KEYS),"files":config.files}),
        );
    }
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
    if !timeout_secs.is_finite() || !(10.0..=3600.0).contains(&timeout_secs) {
        return Err(sc_agent::error(
            "decidr.invalid_request_timeout",
            format!("{timeout_secs}: allowed range 10..=3600 seconds"),
        ));
    }
    if workers.is_some_and(|w| !(1..=64).contains(&w)) || (workers.is_some() && !batch) {
        return Err(sc_agent::error(
            "decidr.invalid_workers",
            "workers requires --batch and a value in 1..=64",
        ));
    }
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
        timeout: Duration::from_secs_f64(timeout_secs),
        base_url: None,
        root,
        log_root: sc_agent::log_root("decidr")?,
        repo,
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
