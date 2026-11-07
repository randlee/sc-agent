//! Thin CLI over [`sc_treesitter`].
//!
//! Argv is parsed by [`cli::parse`] (clap). Success writes JSON (or the
//! `--help` / `--version` text) to stdout and exits 0. Failure writes one
//! `ScAgentErrorResponse` line to stderr and exits 2.

use std::io::{self, Write};
use std::process::ExitCode;

use mimalloc::MiMalloc;
use sc_agent_types::{JevError, ScAgentErrorResponse};
use sc_treesitter::{
    ExtractInput, extract, languages, queries, query_source, subjects_to_json_bytes,
};

use crate::cli::{Command, ExtractArgs, Parsed};

mod cli;
mod git_io;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> ExitCode {
    match cli::parse(std::env::args_os()).and_then(run) {
        Ok(bytes) => {
            if let Err(error) = io::stdout().write_all(&bytes) {
                emit(&JevError::io_failed("stdout", &error));
                return ExitCode::from(2);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            emit(&error);
            ExitCode::from(2)
        }
    }
}

fn emit(error: &JevError) {
    let body = ScAgentErrorResponse::from_jev_error(error);
    let line = serde_json::to_string(&body).unwrap_or_else(|_| {
        serde_json::to_string(&ScAgentErrorResponse::from_jev_error(
            &JevError::extraction_failed("stderr encoding"),
        ))
        .expect("ScAgentErrorResponse from JevError must serialize")
    });
    let _ = writeln!(io::stderr(), "{line}");
}

fn run(parsed: Parsed) -> Result<Vec<u8>, JevError> {
    let cli = match parsed {
        Parsed::Print(text) => return Ok(text.into_bytes()),
        Parsed::Run(cli) => cli,
    };
    match cli.command {
        Command::Languages => {
            let ids: Vec<&str> = languages().map(|info| info.id).collect();
            json_line(&serde_json::json!(ids))
        }
        Command::Queries(args) => {
            let listed = queries(args.language.as_deref())?;
            let value: Vec<serde_json::Value> = listed
                .iter()
                .map(|info| {
                    serde_json::json!({
                        "language": info.language,
                        "query": info.name,
                    })
                })
                .collect();
            json_line(&serde_json::Value::Array(value))
        }
        Command::Extract(args) => run_extract(&args),
    }
}

fn run_extract(args: &ExtractArgs) -> Result<Vec<u8>, JevError> {
    let query_text = query_source(&args.language, &args.query)?;
    let blob = git_io::read_blob(&args.rev, &args.path)?;
    let capture_refs: Option<Vec<&str>> = args
        .captures
        .as_ref()
        .map(|captures| captures.names().collect());
    let document = extract(
        &ExtractInput::new(&args.language, query_text, &blob.bytes)
            .with_captures(capture_refs.as_deref())
            .with_path(blob.path.as_str())
            .with_rev(args.rev.as_str())
            .with_blob_sha(blob.sha.as_str()),
    )?;
    subjects_to_json_bytes(&document)
}

fn json_line(value: &serde_json::Value) -> Result<Vec<u8>, JevError> {
    let mut bytes = serde_json::to_vec(value)
        .map_err(|error| JevError::encode_failed("listing JSON", &error))?;
    bytes.push(b'\n');
    Ok(bytes)
}
