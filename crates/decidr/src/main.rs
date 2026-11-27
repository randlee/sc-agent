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
    let doctor = parsed
        .as_ref()
        .is_ok_and(|cli| matches!(cli.command, Command::Doctor));
    let value = match signal.and_then(|()| sc_agent::default_logger("decidr")) {
        Err(error) => sc_agent::result(Err(error)),
        Ok(logger) => {
            let logger = Arc::new(logger);
            let outcome = (|| -> Result<Value, sc_agent::Error> {
                let cli = parsed.map_err(|e| sc_agent::error("decidr.usage", e))?;
                if matches!(cli.command, Command::Models) {
                    return Ok(json!(sc_agent::PROVIDERS));
                }
                if matches!(cli.command, Command::Doctor) {
                    return sc_agent::doctor(cli.config_dir.as_deref(), cli.project.as_deref());
                }
                let (loaded, root, repo) =
                    sc_agent::load(cli.config_dir.as_deref(), cli.project.as_deref())?;
                let mut settings = loaded.config;
                let Command::Send {
                    model,
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
                cancelled.check()?;
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
                        cancelled
                            .recv(&receiver, "decidr.input_read")?
                            .map_err(|e| sc_agent::error("decidr.input_read", e))?
                    }
                };
                let options = sc_agent::Options {
                    environment: std::env::vars_os().collect(),
                    settings,
                    model,
                    agent_role,
                    timeout: Duration::try_from_secs_f64(timeout_secs)
                        .map_err(|e| sc_agent::error("decidr.invalid_request_timeout", e))?,
                    base_url: None,
                    root,
                    log_root: sc_agent::log_root("decidr")?,
                    repo,
                };
                let run = sc_agent::Run::new(options, Some(logger.clone()), cancelled.clone())?;
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
                    run.send_batch(&requests, &|_, _| {})
                } else {
                    run.send(
                        &serde_json::from_str(&text)
                            .map_err(|e| sc_agent::error("decision.request_parse", e))?,
                    )
                }
            })();
            let value = if doctor {
                sc_agent::result(outcome)
            } else {
                sc_agent::report(&logger, outcome)
            };
            sc_agent::after_shutdown(value, logger.shutdown())
        }
    };
    let status = if value.get("ok") == Some(&Value::Bool(true)) && !cancelled.is_cancelled() {
        0
    } else {
        1
    };
    if writeln!(std::io::stdout().lock(), "{value}").is_err() {
        std::process::exit(1);
    }
    std::process::exit(status);
}
