use clap::Parser;
use cli::{Cli, Command};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    sync::{Arc, mpsc},
    time::Duration,
};
mod cli;
fn main() {
    let cancelled = Arc::new(sc_agent::Cancel::default());
    let flag = cancelled.clone();
    let signal = ctrlc::set_handler(move || flag.cancel())
        .map_err(|error| sc_agent::error("decidr.signal", error));
    let parsed = Cli::try_parse();
    if let Err(error) = &parsed
        && matches!(
            error.kind(),
            clap::error::ErrorKind::DisplayVersion | clap::error::ErrorKind::DisplayHelp
        )
    {
        if error.print().is_err() {
            std::process::exit(1);
        }
        return;
    }
    let value = match signal.and_then(|()| sc_agent::default_logger("decidr")) {
        Err(error) => sc_agent::result(Err(error)),
        Ok(logger) => {
            let outcome = (|| -> Result<Value, sc_agent::Error> {
                let cli = parsed.map_err(|e| sc_agent::error("decidr.usage", e))?;
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
                    settings.workers = sc_agent::workers(workers)?;
                }
                if cancelled.is_cancelled() {
                    return Err(sc_agent::error("decision.cancelled", "interrupted"));
                }
                let text = match input {
                    Some(path) => std::fs::read_to_string(path)
                        .map_err(|e| sc_agent::error("decidr.input_read", e))?,
                    None => {
                        let (sender, receiver) = mpsc::sync_channel(1);
                        std::thread::spawn(move || {
                            let mut text = String::new();
                            let read = std::io::stdin().read_to_string(&mut text).map(|_| text);
                            let _ = sender.send(read);
                        });
                        loop {
                            if cancelled.is_cancelled() {
                                return Err(sc_agent::error("decision.cancelled", "interrupted"));
                            }
                            match receiver.recv_timeout(Duration::from_millis(20)) {
                                Ok(value) => {
                                    break value
                                        .map_err(|e| sc_agent::error("decidr.input_read", e))?;
                                }
                                Err(mpsc::RecvTimeoutError::Timeout) => {}
                                Err(e) => return Err(sc_agent::error("decidr.input_read", e)),
                            }
                        }
                    }
                };
                let log_root = sc_agent::log_root("decidr")?;
                let options = sc_agent::Options {
                    environment: std::env::vars_os().collect(),
                    settings,
                    model,
                    provider,
                    agent_role,
                    timeout: Duration::try_from_secs_f64(timeout_secs)
                        .map_err(|e| sc_agent::error("decidr.invalid_request_timeout", e))?,
                    base_url: None,
                    root,
                    log_root,
                    repo,
                };
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
                        &cancelled,
                        &|delay| cancelled.wait(delay),
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
                        &cancelled,
                        &|delay| cancelled.wait(delay),
                        &sc_agent::os_keyring,
                    )
                };
                if cancelled.is_cancelled() {
                    return Err(sc_agent::error("decision.cancelled", "interrupted"));
                }
                result
            })();
            let value = sc_agent::report(&logger, outcome);
            if let Err(error) = logger.shutdown() {
                eprintln!("{error}");
            }
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
