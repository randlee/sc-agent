//! The `decide` binary: argv -> `decide::{send, send_batch, models, doctor}` -> stdout.
//!
//! Argument parsing and output only; all behaviour lives in the decide library. Exit codes:
//! 0 when the command ran and every result succeeded, 1 otherwise, 130 after an interrupt
//! (REQ-SAG-0011).
//!
//! The binary opens its general and session loggers itself and closes them explicitly before
//! it returns; once it starts opening them, every interrupt takes that close path. Worker threads of the runner are never joined, so the last logger handle may
//! outlive `main`; the explicit close is the flush barrier that puts every record and row on
//! disk. A close failure (lost rows) is reported as one `decide.log` line on stderr and does
//! not change the exit code.

#![forbid(unsafe_code)]

mod cli;

use std::io::{self, BufReader, Read, Write};
use std::process::ExitCode;

use cli::{Command, Parsed, SendArgs};
use decide::input::{build_request, read_batch, request_bytes};
use decide::signal::{EXIT_INTERRUPTED, SignalWatch};
use decide::{
    Context, DecideError, DoctorOptions, ModelSelector, ModelsOptions, SendOptions, doctor,
    general_logger, load_config, models, send, send_batch, session_logger,
};
use sc_agent_log::{GeneralLogger, LogError, Slug};
use sc_agent_types::{CancelToken, ErrorId, Provider};
use sc_decision_schema::{DecisionRequest, DecisionResult};

/// The general-log service of the binary.
const SERVICE: &str = "decide";

fn main() -> ExitCode {
    let cancel = CancelToken::new();
    let watch = match decide::signal::install(&cancel) {
        Ok(watch) => watch,
        Err(error) => return emit(&error),
    };
    let code = match cli::parse(std::env::args_os()) {
        Ok(parsed) => {
            let mut stdin = io::stdin();
            let mut stdout = io::stdout();
            run(parsed, &cancel, &watch, &mut stdin, &mut stdout)
                .unwrap_or_else(|error| emit(&error))
        }
        Err(error) => emit(&error),
    };
    if watch.interrupted() {
        ExitCode::from(u8::try_from(EXIT_INTERRUPTED).unwrap_or(u8::MAX))
    } else {
        code
    }
}

/// Runs one parsed command; results go to `stdout`.
fn run(
    parsed: Parsed,
    cancel: &CancelToken,
    watch: &SignalWatch,
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
) -> Result<ExitCode, DecideError> {
    let cli = match parsed {
        Parsed::Print(text) => return Ok(print_stdout(text.as_bytes())),
        Parsed::Run(cli) => cli,
    };
    let config_dir = cli.config_dir;
    match cli.command {
        Command::Send(args) => run_send(args, config_dir, cancel, watch, stdin, stdout),
        Command::Models(args) => {
            watch.logs_opened();
            let (context, general) = context_with_general(config_dir.clone())?;
            let options = ModelsOptions {
                runner: args.runner,
                context,
            };
            let list = models(&options, cancel);
            drop(options);
            close_general(general);
            write_json(stdout, &list?)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Doctor => {
            watch.logs_opened();
            let (context, general) = context_with_general(config_dir.clone())?;
            let options = DoctorOptions { context };
            let report = doctor(&options, cancel);
            drop(options);
            close_general(general);
            write_json(stdout, &report?)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// What `decide send` read: one request or a batch.
enum Input {
    One(Box<DecisionRequest>),
    Batch(Vec<DecisionRequest>),
}

fn run_send(
    args: SendArgs,
    config_dir: Option<std::path::PathBuf>,
    cancel: &CancelToken,
    watch: &SignalWatch,
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
) -> Result<ExitCode, DecideError> {
    // Slugs first: an invalid one sends and logs nothing (REQ-SAG-0004).
    let project = Slug::new("project", &args.project)?;
    let agent_role = Slug::new("agent_role", &args.agent_role)?;
    let config = load_config(&base_context(config_dir.clone()))?;
    let model = ModelSelector::parse(&args.model, &config)?;
    let provider = args
        .provider
        .as_deref()
        .map(str::parse::<Provider>)
        .transpose()?;
    // The whole input is read and checked before anything is sent (REQ-SAG-0504); until
    // `logs_opened` an interrupt exits at once, so a blocked stdin or FIFO read cannot swallow it.
    let input = if args.batch {
        Input::Batch(if args.input == "-" {
            read_batch(&mut BufReader::new(stdin))?
        } else {
            let file = std::fs::File::open(&args.input)
                .map_err(|error| DecideError::input(&args.input, error))?;
            read_batch(&mut BufReader::new(file))?
        })
    } else {
        let bytes = request_bytes(&args.input, stdin)?;
        Input::One(Box::new(build_request(
            &bytes,
            &args.images,
            args.long_context,
            None,
        )?))
    };
    // From here an interrupt cancels and the run drains, flushes and closes the logs.
    watch.logs_opened();
    let (context, general) = context_with_general(config_dir)?;
    let mut options = SendOptions {
        model,
        provider,
        project,
        agent_role,
        context,
        session: None,
    };
    let session = match session_logger(&options) {
        Ok(session) => session,
        Err(error) => {
            drop(options);
            close_general(general);
            return Err(error);
        }
    };
    options.session = Some(session.clone());
    let outcome = match input {
        Input::Batch(requests) => {
            let mut all_ok = true;
            send_batch(&options, requests, cancel, &mut |result| {
                all_ok &= result.outcome.is_ok();
                write_result(stdout, &result)
            })
            .map(|()| all_ok)
        }
        Input::One(request) => {
            // The result is printed as soon as it exists; a session-log flush failure after
            // it is a command error (exit 1) printed after it, never instead (QA-006).
            let mut success = false;
            send(&options, *request, cancel, &mut |result| {
                success = result.outcome.is_ok();
                write_result(stdout, &result)
            })
            .map(|()| success)
        }
    };
    drop(options);
    report_close(session.close());
    close_general(general);
    outcome.map(exit)
}

/// The binary's context with its general logger opened (and kept, to close it later).
/// The binary's context: `--config-dir` (else `~/.sc-agent`) and every other seam default.
fn base_context(config_dir: Option<std::path::PathBuf>) -> Context {
    let mut context = Context::new(SERVICE);
    context.config_dir = config_dir;
    context
}

fn context_with_general(
    config_dir: Option<std::path::PathBuf>,
) -> Result<(Context, GeneralLogger), DecideError> {
    let mut context = base_context(config_dir);
    let general = general_logger(&context)?;
    context.general = Some(general.clone());
    Ok((context, general))
}

fn close_general(general: GeneralLogger) {
    report_close(general.close());
}

/// One `decide.log` stderr line for a logger that lost rows or could not be flushed; the
/// exit code is not changed.
fn report_close(closed: Result<(), LogError>) {
    if let Err(error) = closed {
        report(&DecideError::from(error));
    }
}

fn exit(success: bool) -> ExitCode {
    if success {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// One result line plus `\n`, flushed so a batch prints each result as it arrives.
fn write_result(stdout: &mut dyn Write, result: &DecisionResult) -> Result<(), DecideError> {
    let mut line = decide::output::result_line(result)?;
    line.push(b'\n');
    write_bytes(stdout, &line)
}

fn write_json<T: serde::Serialize>(stdout: &mut dyn Write, value: &T) -> Result<(), DecideError> {
    let mut text = decide::output::to_json(value)?;
    text.push('\n');
    write_bytes(stdout, text.as_bytes())
}

fn write_bytes(stdout: &mut dyn Write, bytes: &[u8]) -> Result<(), DecideError> {
    stdout
        .write_all(bytes)
        .and_then(|()| stdout.flush())
        .map_err(|source| DecideError::Write { source })
}

fn print_stdout(bytes: &[u8]) -> ExitCode {
    match io::stdout().write_all(bytes) {
        Ok(()) => ExitCode::SUCCESS,
        Err(source) => emit(&DecideError::Write { source }),
    }
}

/// One stderr line: `error.detail()` as JSON (`recovery` included when present).
fn report(error: &DecideError) {
    let line = serde_json::to_string(&error.detail()).unwrap_or_else(|_| {
        r#"{"id":"decide.encode","message":"the error could not be encoded as JSON"}"#.to_owned()
    });
    // Best effort: a closed stderr leaves nowhere to report the failure.
    let _ = writeln!(io::stderr(), "{line}");
}

/// [`report`], then exit 1.
fn emit(error: &DecideError) -> ExitCode {
    report(error);
    ExitCode::FAILURE
}
