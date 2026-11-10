//! `tsq`: run predefined tree-sitter queries from the command line.
//!
//! Output: JSON plus a newline on stdout, exit 0. Failure: one stderr line
//! `{"id": "treesitter.<variant>", "message": ...}`, exit 1, empty stdout. Each run records
//! one `tsq.command` event in its own general log, `~/.sc-agent/logs/tsq.log.jsonl`.
#![forbid(unsafe_code)]

mod cli;

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use sc_observability::v2::{Logger, LoggerConfig};
use sc_observability::{
    ActionName, Level, LogEvent, OBSERVATION_ENVELOPE_VERSION, OutcomeLabel, ProcessIdentity,
    SchemaVersion, ServiceName, TargetCategory, Timestamp,
};
use sc_treesitter::{Error, queries, query_source, read_source, run_query, to_json_bytes};

use crate::cli::{Command, Parsed};

/// Service name, and so the log file name `tsq.log.jsonl`.
const SERVICE: &str = "tsq";

fn main() -> ExitCode {
    let logger = match general_log() {
        Ok(logger) => logger,
        Err(error) => return emit(&error),
    };
    let parsed = cli::parse(std::env::args_os());
    let subcommand = match &parsed {
        Ok(Parsed::Run(cli)) => cli.command.name(),
        Ok(Parsed::Print(_)) => "help",
        Err(_) => "usage",
    };
    let result = parsed.and_then(run).and_then(|bytes| {
        io::stdout()
            .lock()
            .write_all(&bytes)
            .and_then(|()| io::stdout().lock().flush())
            .map_err(|error| Error::Write {
                kind: format!("{:?}", error.kind()),
                detail: error.to_string(),
            })
    });
    record(&logger, subcommand, result.as_ref().err());
    // A log failure never changes the result.
    let _shutdown = logger.shutdown();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => emit(&error),
    }
}

fn run(parsed: Parsed) -> Result<Vec<u8>, Error> {
    let cli = match parsed {
        Parsed::Print(text) => return Ok(text.into_bytes()),
        Parsed::Run(cli) => cli,
    };
    match cli.command {
        Command::Languages => {
            let ids: Vec<&str> = sc_treesitter::languages().collect();
            json_line(&ids)
        }
        Command::Queries(args) => json_line(&queries(args.language.as_deref())?),
        Command::Query(args) => {
            let capture_refs: Option<Vec<&str>> = args
                .captures
                .as_ref()
                .map(|captures| captures.names().collect());
            let text = query_source(&args.language, &args.query)?;
            let bytes = read_source(&args.file)?;
            let captures = run_query(
                &args.language,
                text,
                &bytes,
                Some(&args.file),
                capture_refs.as_deref(),
            )?;
            to_json_bytes(&captures)
        }
    }
}

/// Compact JSON plus a trailing newline.
fn json_line<T: serde::Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, Error> {
    let mut bytes = serde_json::to_vec(value).map_err(|error| Error::Encode {
        detail: error.to_string(),
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// One stderr line: the serialized Error `{"id": ..., "message": ...}`; exit 1.
///
/// Nothing is left to report to if stderr itself cannot be written.
fn emit(error: &Error) -> ExitCode {
    let line = json_line(error).unwrap_or_else(|_| {
        // Serializing two strings cannot fail; keep a well-formed line regardless.
        format!("{{\"id\":\"{}\",\"message\":\"\"}}\n", error.id()).into_bytes()
    });
    let mut stderr = io::stderr().lock();
    let _written = stderr.write_all(&line).and_then(|()| stderr.flush());
    ExitCode::FAILURE
}

/// sc-observability logger, service `tsq`, log root `home_dir()/.sc-agent`.
///
/// # Errors
///
/// [`Error::Log`] when the home directory is unknown or the logger cannot start.
fn general_log() -> Result<Logger, Error> {
    let home = std::env::home_dir()
        .filter(|home| !home.as_os_str().is_empty())
        .ok_or_else(|| Error::Log {
            detail: "the home directory is unknown".to_owned(),
        })?;
    let service = ServiceName::new(SERVICE).map_err(|error| Error::Log {
        detail: error.to_string(),
    })?;
    let config = LoggerConfig::default_for(service, log_root(home));
    Logger::new(config).map_err(|error| Error::Log {
        detail: format!("{}: {}", error.code(), error.diagnostic().message),
    })
}

fn log_root(home: PathBuf) -> PathBuf {
    home.join(".sc-agent")
}

/// Records the one `tsq.command` event; a failure here is dropped (it never changes the result).
fn record(logger: &Logger, subcommand: &str, error: Option<&Error>) {
    if let Some(event) = command_event(logger, subcommand, error) {
        let _logged = logger.log(event);
    }
}

fn command_event(logger: &Logger, subcommand: &str, error: Option<&Error>) -> Option<LogEvent> {
    let mut fields = serde_json::Map::new();
    fields.insert("subcommand".to_owned(), subcommand.into());
    fields.insert(
        "error_id".to_owned(),
        error.map_or(serde_json::Value::Null, |error| error.id().into()),
    );
    Some(LogEvent {
        version: SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION).ok()?,
        timestamp: Timestamp::now_utc(),
        level: if error.is_some() {
            Level::Error
        } else {
            Level::Info
        },
        service: logger.service_name().clone(),
        target: TargetCategory::new(SERVICE).ok()?,
        action: ActionName::new("tsq.command").ok()?,
        message: error.map(Error::message),
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: Some(OutcomeLabel::new(if error.is_some() { "error" } else { "ok" }).ok()?),
        diagnostic: None,
        state_transition: None,
        fields,
    })
}
