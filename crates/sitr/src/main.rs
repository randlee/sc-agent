//! `sitr`: run predefined tree-sitter queries from the command line.
//!
//! Output: JSON plus a newline on stdout, exit 0. Failure: one stderr line
//! `{"id": "treesitter.<variant>", "message": ...}`, exit 1, empty stdout. Each run records
//! one `sitr.command` event in its own general log, `~/.sc-agent/logs/sitr.log.jsonl`, or
//! `<PATH>/logs/sitr.log.jsonl` with `--log-root <PATH>`.
#![forbid(unsafe_code)]

mod cli;

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use sc_observability::v2::{Logger, LoggerConfig};
use sc_observability::{
    ActionName, Level, LogEvent, OBSERVATION_ENVELOPE_VERSION, OutcomeLabel, ProcessIdentity,
    SchemaVersion, ServiceName, TargetCategory, Timestamp,
};
use sc_treesitter::{
    Error, LanguageId, queries, query_source, read_source, run_query, to_json_bytes,
};

use crate::cli::{Command, Parsed};

/// Service name, and so the log file name `sitr.log.jsonl`.
const SERVICE: &str = "sitr";

/// What the `sitr.command` event records about one run.
#[derive(Debug, Default)]
struct CommandFields {
    subcommand: &'static str,
    language: Option<String>,
    query: Option<String>,
    file: Option<PathBuf>,
    bytes: Option<usize>,
}

fn main() -> ExitCode {
    let started = Instant::now();
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let parsed = cli::parse(args.clone());
    let flag = match &parsed {
        Ok(Parsed::Run(cli)) => cli.log_root.clone(),
        Ok(Parsed::Print(_)) | Err(_) => cli::log_root_arg(&args),
    };
    let logger = match general_log(flag) {
        Ok(logger) => logger,
        Err(error) => return emit(&error),
    };
    let mut fields = CommandFields {
        subcommand: match &parsed {
            Ok(Parsed::Run(cli)) => cli.command.name(),
            Ok(Parsed::Print(_)) => "help",
            Err(_) => "usage",
        },
        ..CommandFields::default()
    };
    let result = parsed
        .and_then(|parsed| run(parsed, &mut fields))
        .and_then(|bytes| {
            let mut stdout = io::stdout().lock();
            stdout
                .write_all(&bytes)
                .and_then(|()| stdout.flush())
                .map_err(|error| Error::Write {
                    kind: error.kind(),
                    detail: error.to_string(),
                })
        });
    record(&logger, &fields, started, result.as_ref().err());
    // A log failure never changes the result.
    let _shutdown = logger.shutdown();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => emit(&error),
    }
}

fn run(parsed: Parsed, fields: &mut CommandFields) -> Result<Vec<u8>, Error> {
    let cli = match parsed {
        Parsed::Print(text) => return Ok(text.into_bytes()),
        Parsed::Run(cli) => cli,
    };
    match cli.command {
        Command::Languages => {
            let ids: Vec<LanguageId> = sc_treesitter::languages().collect();
            json_line(&ids)
        }
        Command::Queries(args) => {
            fields.language.clone_from(&args.language);
            let language = args.language.as_deref().map(LanguageId::new).transpose()?;
            json_line(&queries(language))
        }
        Command::Query(args) => {
            fields.language = Some(args.language.clone());
            fields.query = Some(args.query.clone());
            fields.file = Some(args.file.clone());
            let capture_refs: Option<Vec<&str>> = args
                .captures
                .as_ref()
                .map(|captures| captures.names().collect());
            let language = LanguageId::new(&args.language)?;
            let text = query_source(language, &args.query)?;
            let bytes = read_source(&args.file)?;
            fields.bytes = Some(bytes.len());
            let captures = run_query(
                language,
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

/// One stderr line: the serialized Error `{"id", "message", "recovery"?}`; exit 1.
///
/// A failed stderr write is dropped on purpose: stderr is the last channel, so there is
/// nowhere to report it, and exit code 1 is the only signal left.
fn emit(error: &Error) -> ExitCode {
    let line = json_line(error).unwrap_or_else(|_| {
        // Serializing two strings cannot fail; keep a well-formed line regardless.
        format!("{{\"id\":\"{}\",\"message\":\"\"}}\n", error.id()).into_bytes()
    });
    let mut stderr = io::stderr().lock();
    let _written = stderr.write_all(&line).and_then(|()| stderr.flush());
    ExitCode::FAILURE
}

/// sc-observability logger, service `sitr`, log root `log_root` (`--log-root`), else
/// `home_dir()/.sc-agent`.
///
/// # Errors
///
/// [`Error::Log`] when no log root is given and the home directory is unknown, or the logger
/// cannot start.
fn general_log(log_root: Option<PathBuf>) -> Result<Logger, Error> {
    let root = match log_root {
        Some(root) => root,
        None => std::env::home_dir()
            .filter(|home| !home.as_os_str().is_empty())
            .map(default_log_root)
            .ok_or_else(|| Error::Log {
                detail: "the home directory is unknown".to_owned(),
            })?,
    };
    let service = ServiceName::new(SERVICE).map_err(|error| Error::Log {
        detail: error.to_string(),
    })?;
    let config = LoggerConfig::default_for(service, root);
    Logger::new(config).map_err(|error| Error::Log {
        detail: format!("{}: {}", error.code(), error.diagnostic().message),
    })
}

fn default_log_root(home: PathBuf) -> PathBuf {
    home.join(".sc-agent")
}

/// Records the one `sitr.command` event; a failure here is dropped (it never changes the result).
fn record(logger: &Logger, command: &CommandFields, started: Instant, error: Option<&Error>) {
    if let Some(event) = command_event(logger, command, started, error) {
        let _logged = logger.log(event);
    }
}

fn command_event(
    logger: &Logger,
    command: &CommandFields,
    started: Instant,
    error: Option<&Error>,
) -> Option<LogEvent> {
    let optional = |value: Option<serde_json::Value>| value.unwrap_or(serde_json::Value::Null);
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let mut fields = serde_json::Map::new();
    fields.insert("subcommand".to_owned(), command.subcommand.into());
    fields.insert(
        "language".to_owned(),
        optional(command.language.clone().map(Into::into)),
    );
    fields.insert(
        "query".to_owned(),
        optional(command.query.clone().map(Into::into)),
    );
    fields.insert(
        "file".to_owned(),
        optional(
            command
                .file
                .as_ref()
                .map(|file| file.display().to_string().into()),
        ),
    );
    fields.insert("bytes".to_owned(), optional(command.bytes.map(Into::into)));
    fields.insert("duration_ms".to_owned(), duration_ms.into());
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
        action: ActionName::new("sitr.command").ok()?,
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
