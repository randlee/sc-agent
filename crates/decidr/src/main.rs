#![forbid(unsafe_code)]
//! decidr: send decisions from the command line.

use std::io::{BufRead, Read, Write};
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};
use sc_agent_config::{KEYS, config, repo_root};
use sc_agent_http::{Cancel, Policy};
use sc_agent_log::{Session, general, log_root, record, repo_id};
use sc_agent_secrets as keys;
use sc_agent_types::Error;
use sc_agent_types::providers::{MODELS, PROVIDERS};
use sc_config::layers;
use sc_decision_local::Local;
use sc_decision_runner::{Decider, Decision, Outcome};
use sc_decision_schema::DecisionRequest;
use sc_observability::v2::Logger;
use serde_json::{Map, json};

#[derive(Parser)]
#[command(name = "decidr", version)]
struct Cli {
    #[arg(long, global = true)]
    project: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Decide the request on stdin (or a JSONL batch with --batch).
    Send {
        #[arg(long)]
        model: String,
        #[arg(long)]
        provider: Option<String>,
        /// A local runner named in config.
        #[arg(long, conflicts_with = "provider")]
        runner: Option<String>,
        #[arg(long)]
        agent_role: String,
        #[arg(long)]
        batch: bool,
        #[arg(long, requires = "batch")]
        workers: Option<usize>,
        #[arg(long)]
        timeout_secs: Option<u64>,
    },
    /// The model table.
    Models,
    /// Settings with their sources, and where each provider key comes from.
    Doctor,
}

/// The entry point owns the process's one Logger (ADR-SAG-0021): `logs/decidr.log.jsonl`
/// under [`log_root`]. Every send and every failed command is recorded to it.
fn main() -> ExitCode {
    let cli = Cli::parse();
    let opened = log_root("decidr").and_then(|dir| Ok((general("decidr", &dir)?, dir)));
    let (logger, log_dir) = match opened {
        Ok(opened) => opened,
        Err(error) => return fail(&error),
    };
    let root = repo_root();
    let result = match cli.command {
        Command::Models => models(),
        Command::Doctor => doctor(cli.project.as_deref(), root.as_deref()),
        Command::Send {
            model,
            provider,
            runner,
            agent_role,
            batch,
            workers,
            timeout_secs,
        } => send(
            cli.project.as_deref(),
            root.as_deref(),
            &model,
            provider.as_deref(),
            runner.as_deref(),
            &agent_role,
            batch,
            workers,
            timeout_secs,
            &logger,
            &log_dir,
        ),
    };
    let code = result.unwrap_or_else(|error| {
        let mut fields = Map::new();
        fields.insert("error_id".into(), error.id().into());
        fields.insert("message".into(), error.to_string().into());
        record(&logger, "sc_agent.decidr", "decidr.error", fields);
        fail(&error)
    });
    let _ = logger.flush();
    code
}

fn fail(error: &Error) -> ExitCode {
    println!(
        "{}",
        json!({ "ok": false, "error": { "id": error.id(), "message": error.to_string() } })
    );
    ExitCode::FAILURE
}

#[expect(clippy::too_many_arguments, reason = "the send command's flags")]
fn send(
    project: Option<&str>,
    root: Option<&Path>,
    model: &str,
    provider: Option<&str>,
    runner: Option<&str>,
    agent_role: &str,
    batch: bool,
    workers: Option<usize>,
    timeout_secs: Option<u64>,
    logger: &Logger,
    log_root: &Path,
) -> Result<ExitCode, Error> {
    let layered = config(project, root)?;
    let config = layered.config;
    let project = config.project.as_deref().ok_or_else(|| Error::Config("project is required: --project, SC_PROJECT, or \"project\" in sc-agent.json or decidr.json".into()))?;
    let policy = Policy {
        max_sends: config.max_sends.unwrap_or(5),
        timeout: Duration::from_secs(timeout_secs.or(config.request_timeout_secs).unwrap_or(120)),
        ..Policy::default()
    };
    let requests = read_requests(batch)?;
    let session = Session::open(
        log_root,
        repo_id(&std::env::current_dir().unwrap_or_default()),
        project,
        agent_role,
    )?;
    let (decision, model_id): (Box<dyn Decision>, String) = match runner {
        Some(name) => {
            let r = config
                .local_runners
                .get(name)
                .ok_or_else(|| Error::Config(format!("no local runner {name} in config")))?;
            let base = format!("http://{}:{}", r.host, r.port);
            let local = Local::new(
                &base,
                &r.kind,
                |s| Decider::new(s, policy, Some(session)),
                Duration::from_secs(r.load_timeout_secs),
            )?;
            (Box::new(local), model.to_owned())
        }
        None => decidr::open(model, provider, root, policy, Some(session))?,
    };
    let requests = requests
        .into_iter()
        .map(|r| DecisionRequest {
            model: model_id.clone(),
            ..r
        })
        .collect();
    let cancel = Cancel::default();
    let handler = cancel.clone();
    let _ = ctrlc::set_handler(move || handler.cancel());
    let mut failed = false;
    let mut out = std::io::stdout().lock();
    decision.decide_batch(requests, workers.or(config.workers).unwrap_or(8), &cancel, &mut |outcome: Outcome| {
        failed |= outcome.result.is_err();
        let mut fields = Map::new();
        fields.insert("model".into(), model_id.clone().into());
        fields.insert("position".into(), outcome.position.into());
        fields.insert("session_id".into(), outcome.session.to_string().into());
        fields.insert("ok".into(), outcome.result.is_ok().into());
        fields.insert("error_id".into(), outcome.result.as_ref().err().map(|e| e.id()).into());
        record(logger, "sc_agent.decidr", "decidr.send", fields);
        let line = match outcome.result {
            Ok(value) => json!({ "position": outcome.position, "session_id": outcome.session.to_string(), "ok": true, "value": value }),
            Err(e) => json!({ "position": outcome.position, "session_id": outcome.session.to_string(), "ok": false, "error": { "id": e.id(), "message": e.to_string() } }),
        };
        let _ = writeln!(out, "{line}");
    });
    Ok(if cancel.is_cancelled() {
        ExitCode::from(130)
    } else if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// One request, or one per line with `--batch`; every line is checked before anything is sent.
fn read_requests(batch: bool) -> Result<Vec<DecisionRequest>, Error> {
    let stdin = std::io::stdin().lock();
    let parse = |text: &str, line: usize| {
        serde_json::from_str::<DecisionRequest>(text)
            .map_err(|e| Error::Config(format!("request on line {line}: {e}")))
    };
    if !batch {
        let mut text = String::new();
        stdin
            .take(8 * 1024 * 1024 + 1)
            .read_to_string(&mut text)
            .map_err(|e| Error::Config(e.to_string()))?;
        return parse(&text, 1).map(|r| vec![r]);
    }
    stdin
        .lines()
        .enumerate()
        .filter(|(_, line)| line.as_ref().map_or(true, |l| !l.trim().is_empty()))
        .map(|(i, line)| parse(&line.map_err(|e| Error::Config(e.to_string()))?, i + 1))
        .collect()
}

fn models() -> Result<ExitCode, Error> {
    for (short, id, released) in MODELS {
        let served: Vec<&str> = PROVIDERS
            .iter()
            .filter(|p| !p.serves.is_empty() && p.serves(id))
            .map(|p| p.name)
            .collect();
        println!(
            "{}",
            json!({ "short_name": short, "id": id, "released": released, "providers": served })
        );
    }
    Ok(ExitCode::SUCCESS)
}

fn doctor(project: Option<&str>, root: Option<&Path>) -> Result<ExitCode, Error> {
    let layered = config(project, root)?;
    for row in layers::resolved(&layered, KEYS) {
        println!(
            "{}",
            json!({ "setting": row.path, "value": row.value, "source": row.source })
        );
    }
    for file in &layered.files {
        println!("{}", json!({ "file": file.path, "exists": file.exists }));
    }
    for provider in &PROVIDERS {
        let source = match keys::key(provider.key_var, root, None) {
            Ok(Some((_, source))) => source.as_str().to_owned(),
            Ok(None) => "missing".to_owned(),
            Err(e) => format!("error: {e}"),
        };
        println!(
            "{}",
            json!({ "provider": provider.name, "key": provider.key_var, "source": source })
        );
    }
    Ok(ExitCode::SUCCESS)
}
