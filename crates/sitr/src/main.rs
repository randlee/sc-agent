//! `sitr`: run predefined tree-sitter queries from the command line.
//!
//! Output: JSON plus a newline on stdout, exit 0. Failure: one stderr line
//! `{"id": "treesitter.<variant>", "message": ...}`, exit 1, empty stdout. Each run records
//! one `sitr.command` event in its own general log, `~/.sc-agent/logs/sitr.log.jsonl`, or
//! `<PATH>/logs/sitr.log.jsonl` with `--log-root <PATH>`.
#![forbid(unsafe_code)]

mod cli;

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use sc_agent_config::{CliName, LoadOptions, Loaded, doctor_rows, load_layered};
use sc_agent_log::{GeneralLogger, Level, LogContext, default_log_root};
use sc_treesitter::{
    Error, LanguageId, queries, query_source, read_source, run_query, to_json_bytes,
};

use crate::cli::{Command, Parsed};

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
    match parsed {
        Ok(Parsed::Print(text)) => write_result(Ok(text.into_bytes())),
        Ok(Parsed::Run(cli)) => {
            let loaded = match load_config(cli.log_root.as_deref()) {
                Ok(loaded) => loaded,
                Err(error) => return emit(&error),
            };
            if matches!(&cli.command, Command::Doctor) {
                let result = doctor_report(&loaded);
                return write_result(result);
            }
            let logger = match general_log(loaded.config.log_root.as_deref()) {
                Ok(logger) => logger,
                Err(error) => return emit(&error),
            };
            let mut fields = CommandFields {
                subcommand: cli.command.name(),
                ..CommandFields::default()
            };
            let result = run(cli.command, &mut fields)
                .and_then(|bytes| write_output(&bytes, &mut io::stdout().lock()));
            record(&logger, &fields, started, result.as_ref().err());
            let _closed = logger.close();
            finish(result, &mut io::stderr().lock())
        }
        Err(error) => {
            if let Ok(loaded) = load_config(cli::log_root_arg(&args).as_deref())
                && let Ok(logger) = general_log(loaded.config.log_root.as_deref())
            {
                let fields = CommandFields {
                    subcommand: "usage",
                    ..CommandFields::default()
                };
                record(&logger, &fields, started, Some(&error));
                let _closed = logger.close();
            }
            emit(&error)
        }
    }
}

fn write_result(result: Result<Vec<u8>, Error>) -> ExitCode {
    let result = result.and_then(|bytes| write_output(&bytes, &mut io::stdout().lock()));
    finish(result, &mut io::stderr().lock())
}

fn write_output(bytes: &[u8], output: &mut impl Write) -> Result<(), Error> {
    output
        .write_all(bytes)
        .and_then(|()| output.flush())
        .map_err(|error| Error::Write {
            kind: error.kind(),
            detail: error.to_string(),
        })
}

fn finish(result: Result<(), Error>, stderr: &mut impl Write) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => emit_to(&error, stderr),
    }
}

fn run(command: Command, fields: &mut CommandFields) -> Result<Vec<u8>, Error> {
    match command {
        Command::Doctor => Err(Error::Usage {
            kind: "Internal".to_owned(),
            detail: "doctor must run without opening a logger".to_owned(),
        }),
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
    emit_to(error, &mut io::stderr().lock())
}

fn emit_to(error: &Error, stderr: &mut impl Write) -> ExitCode {
    let line = json_line(error).unwrap_or_else(|_| {
        // Serializing two strings cannot fail; keep a well-formed line regardless.
        format!("{{\"id\":\"{}\",\"message\":\"\"}}\n", error.id()).into_bytes()
    });
    let _written = stderr.write_all(&line).and_then(|()| stderr.flush());
    ExitCode::FAILURE
}

/// Loads all sitr settings from the shared seven-layer resolver.
fn load_config(log_root_flag: Option<&Path>) -> Result<Loaded, Error> {
    let home = sc_agent_log::resolve_home().ok_or_else(|| Error::Config {
        detail: "home directory is unknown".to_owned(),
    })?;
    let cwd = std::env::current_dir().map_err(|error| Error::Config {
        detail: format!("cannot determine current directory: {error}"),
    })?;
    let worktree = sc_agent_log::Repo::work_tree_root(&cwd).map_err(|error| Error::Config {
        detail: error.to_string(),
    })?;
    let flags = log_root_flag.map_or_else(BTreeMap::new, |path| {
        BTreeMap::from([("log_root".to_owned(), path.display().to_string().into())])
    });
    load_layered(&LoadOptions {
        cli: CliName::Sitr,
        home: home.clone(),
        worktree,
        default_log_root: default_log_root(&home),
        flags,
        environment: std::env::vars_os().collect(),
    })
    .map_err(|error| Error::Config {
        detail: error.to_string(),
    })
}

/// The shared sc-agent-log default for service `sitr`.
fn general_log(log_root: Option<&Path>) -> Result<GeneralLogger, Error> {
    GeneralLogger::open_default(log_root, "sitr").map_err(|error| Error::Log {
        detail: error.to_string(),
    })
}

fn doctor_report(loaded: &Loaded) -> Result<Vec<u8>, Error> {
    #[derive(serde::Serialize)]
    struct Report<'a> {
        settings: Vec<sc_agent_config::DoctorRow>,
        files: &'a [sc_agent_config::LayerFile],
    }
    json_line(&Report {
        settings: doctor_rows(loaded),
        files: &loaded.files,
    })
}

/// Records the one `sitr.command` event; a failure here is dropped (it never changes the result).
fn record(
    logger: &GeneralLogger,
    command: &CommandFields,
    started: Instant,
    error: Option<&Error>,
) {
    let mut fields = serde_json::Map::new();
    fields.insert("subcommand".to_owned(), command.subcommand.into());
    fields.insert(
        "language".to_owned(),
        command
            .language
            .clone()
            .map_or(serde_json::Value::Null, Into::into),
    );
    fields.insert(
        "query".to_owned(),
        command
            .query
            .clone()
            .map_or(serde_json::Value::Null, Into::into),
    );
    fields.insert(
        "file".to_owned(),
        command
            .file
            .as_ref()
            .map_or(serde_json::Value::Null, |file| {
                file.display().to_string().into()
            }),
    );
    fields.insert(
        "bytes".to_owned(),
        command.bytes.map_or(serde_json::Value::Null, Into::into),
    );
    fields.insert(
        "duration_ms".to_owned(),
        u64::try_from(started.elapsed().as_millis())
            .unwrap_or(u64::MAX)
            .into(),
    );
    fields.insert(
        "error_id".to_owned(),
        error.map_or(serde_json::Value::Null, |error| error.id().into()),
    );
    fields.insert(
        "outcome".to_owned(),
        if error.is_some() { "error" } else { "ok" }.into(),
    );
    let _logged = logger.write(
        if error.is_some() {
            Level::Error
        } else {
            Level::Info
        },
        "sitr",
        "sitr.command",
        LogContext::NONE,
        fields,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    struct BrokenWriter;

    impl Write for BrokenWriter {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn closed_stdout_is_a_write_error() {
        let result = write_output(b"[]\n", &mut BrokenWriter);
        let mut stderr = Vec::new();
        assert_eq!(finish(result, &mut stderr), ExitCode::FAILURE);
        let value: serde_json::Value = serde_json::from_slice(&stderr).expect("stderr json");
        assert_eq!(value["id"], "treesitter.write");
        assert!(
            value["message"]
                .as_str()
                .is_some_and(|message| message.contains("broken pipe")),
            "{value}"
        );
    }
}
