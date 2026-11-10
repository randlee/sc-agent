//! The `decide` binary: argv -> `decide::{send, send_batch, models, doctor}` -> stdout.
//!
//! Argument parsing and output only; all behaviour lives in the decide library. Exit codes:
//! 0 when the command ran and every result succeeded, 1 otherwise, 130 after an interrupt
//! (REQ-SAG-0011).

#![forbid(unsafe_code)]

mod cli;

use std::io::{self, BufReader, Read, Write};
use std::process::ExitCode;

use cli::{Command, Parsed};
use decide::input::{build_request, read_batch, request_bytes};
use decide::{
    Context, DecideError, DoctorOptions, ModelSelector, ModelsOptions, SendOptions, doctor,
    load_config, models, send, send_batch,
};
use sc_agent_log::Slug;
use sc_agent_types::{CancelToken, ErrorId, Provider};
use sc_decision_schema::DecisionResult;

/// The exit code after any interrupt (SIGINT, SIGTERM, Ctrl-C, Ctrl-Break).
const EXIT_INTERRUPTED: u8 = 130;

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
            run(parsed, &cancel, &mut stdin, &mut stdout).unwrap_or_else(|error| emit(&error))
        }
        Err(error) => emit(&error),
    };
    if watch.interrupted() {
        ExitCode::from(EXIT_INTERRUPTED)
    } else {
        code
    }
}

/// Runs one parsed command; results go to `stdout`.
fn run(
    parsed: Parsed,
    cancel: &CancelToken,
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
) -> Result<ExitCode, DecideError> {
    let cli = match parsed {
        Parsed::Print(text) => return Ok(print_stdout(text.as_bytes())),
        Parsed::Run(cli) => cli,
    };
    match cli.command {
        Command::Send(args) => {
            let project = Slug::new("project", &args.project).map_err(DecideError::from)?;
            let agent_role =
                Slug::new("agent_role", &args.agent_role).map_err(DecideError::from)?;
            let context = Context::new(SERVICE);
            let config = load_config(&context)?;
            let model = ModelSelector::parse(&args.model, &config)?;
            let provider = args
                .provider
                .as_deref()
                .map(str::parse::<Provider>)
                .transpose()
                .map_err(DecideError::Types)?;
            let options = SendOptions {
                model,
                provider,
                project,
                agent_role,
                context,
                session: None,
            };
            if args.batch {
                let requests = if args.input == "-" {
                    read_batch(&mut BufReader::new(stdin))?
                } else {
                    let file = std::fs::File::open(&args.input)
                        .map_err(|error| input_error(&args.input, &error))?;
                    read_batch(&mut BufReader::new(file))?
                };
                let mut all_ok = true;
                send_batch(&options, requests, cancel, &mut |result| {
                    all_ok &= result.outcome.is_ok();
                    write_result(stdout, &result)
                })?;
                Ok(exit(all_ok))
            } else {
                let bytes = request_bytes(&args.input, stdin)?;
                let request = build_request(&bytes, &args.images, args.long_context, None)?;
                let result = send(&options, request, cancel)?;
                write_result(stdout, &result)?;
                Ok(exit(result.outcome.is_ok()))
            }
        }
        Command::Models(args) => {
            let options = ModelsOptions {
                runner: args.runner,
                context: Context::new(SERVICE),
            };
            let list = models(&options, cancel)?;
            write_json(stdout, &list)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Doctor => {
            let options = DoctorOptions {
                context: Context::new(SERVICE),
            };
            let report = doctor(&options, cancel)?;
            write_json(stdout, &report)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// `InputNotFound` / `InputRead` for a batch file that cannot be opened.
fn input_error(input: &str, error: &io::Error) -> DecideError {
    if error.kind() == io::ErrorKind::NotFound {
        DecideError::InputNotFound {
            input: input.to_owned(),
        }
    } else {
        DecideError::InputRead {
            input: input.to_owned(),
            kind: format!("{:?}", error.kind()),
            detail: error.to_string(),
        }
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
    let mut bytes = serde_json::to_vec(value).map_err(|error| DecideError::Encode {
        detail: error.to_string(),
    })?;
    bytes.push(b'\n');
    write_bytes(stdout, &bytes)
}

fn write_bytes(stdout: &mut dyn Write, bytes: &[u8]) -> Result<(), DecideError> {
    stdout
        .write_all(bytes)
        .and_then(|()| stdout.flush())
        .map_err(|error| DecideError::Write {
            kind: format!("{:?}", error.kind()),
            detail: error.to_string(),
        })
}

fn print_stdout(bytes: &[u8]) -> ExitCode {
    match io::stdout().write_all(bytes) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => emit(&DecideError::Write {
            kind: format!("{:?}", error.kind()),
            detail: error.to_string(),
        }),
    }
}

/// One stderr line: `error.detail()` as JSON (`recovery` included when present); exit 1.
fn emit(error: &DecideError) -> ExitCode {
    let line = serde_json::to_string(&error.detail()).unwrap_or_else(|_| {
        r#"{"id":"decide.encode","message":"the error could not be encoded as JSON"}"#.to_owned()
    });
    // Best effort: a closed stderr leaves nowhere to report the failure.
    let _ = writeln!(io::stderr(), "{line}");
    ExitCode::FAILURE
}
