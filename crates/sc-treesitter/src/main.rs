//! Thin CLI over [`sc_treesitter`].
//!
//! Success writes JSON to stdout. Failure writes one `JevError` object to
//! stderr and exits 2.

use std::io::{self, Write};
use std::process::ExitCode;

use mimalloc::MiMalloc;
use sc_agent_types::{JevError, Rev, ScAgentErrorResponse};
use sc_treesitter::{
    ExtractInput, PRODUCER_NAME, extract, languages, queries, query_source, subjects_to_json_bytes,
};

mod git_io;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

const HELP: &str = "\
sc-treesitter extract --language L --query Q [--captures name,name] [--rev HEAD] <path>
sc-treesitter languages
sc-treesitter queries [--language L]
sc-treesitter --version

<path> is repo-relative, or absolute inside the repository. --rev HEAD reads the
blob with `git cat-file`. --rev WORKTREE reads the working tree. A revision may
not start with `-`. --captures is an allow-list. Execution always uses matches
mode.
";

fn main() -> ExitCode {
    match dispatch(std::env::args().skip(1)) {
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

fn dispatch(args: impl Iterator<Item = String>) -> Result<Vec<u8>, JevError> {
    match parse(args)? {
        Cli::Help => Ok(HELP.as_bytes().to_vec()),
        Cli::Version => Ok(format!("{PRODUCER_NAME} {}\n", env!("CARGO_PKG_VERSION")).into_bytes()),
        Cli::Languages => {
            let ids: Vec<&str> = languages().map(|info| info.id).collect();
            json_line(&serde_json::json!(ids))
        }
        Cli::Queries { language } => {
            let listed = queries(language.as_deref())?;
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
        Cli::Extract {
            language,
            query,
            captures,
            rev,
            path,
        } => {
            let query_text = query_source(&language, &query)?;
            let blob = git_io::read_blob(&rev, &path)?;
            let capture_refs: Vec<&str> = captures.iter().map(String::as_str).collect();
            let allow = if captures.is_empty() {
                None
            } else {
                Some(capture_refs.as_slice())
            };
            let document = extract(
                &ExtractInput::new(&language, query_text, &blob.bytes)
                    .with_captures(allow)
                    .with_path(&blob.path)
                    .with_rev(rev.as_str())
                    .with_blob_sha(&blob.sha),
            )?;
            subjects_to_json_bytes(&document)
        }
    }
}

fn json_line(value: &serde_json::Value) -> Result<Vec<u8>, JevError> {
    let mut bytes = serde_json::to_vec(value)
        .map_err(|error| JevError::encode_failed("listing JSON", &error))?;
    bytes.push(b'\n');
    Ok(bytes)
}

enum Cli {
    Help,
    Version,
    Languages,
    Queries {
        language: Option<String>,
    },
    Extract {
        language: String,
        query: String,
        captures: Vec<String>,
        rev: Rev,
        path: String,
    },
}

fn parse(args: impl Iterator<Item = String>) -> Result<Cli, JevError> {
    let args: Vec<String> = args.collect();
    // `--help` counts only as the command or the first argument after it, so
    // a value such as a path named `-h` later on is not swallowed.
    let is_help = |arg: &String| arg == "--help" || arg == "-h";
    if args.is_empty() || args.iter().take(2).any(is_help) {
        return Ok(Cli::Help);
    }
    match args[0].as_str() {
        "help" => Ok(Cli::Help),
        "--version" | "-V" => {
            if args.len() != 1 {
                return Err(JevError::input_invalid("--version does not take arguments"));
            }
            Ok(Cli::Version)
        }
        "languages" => {
            if args.len() != 1 {
                return Err(JevError::input_invalid("languages does not take arguments"));
            }
            Ok(Cli::Languages)
        }
        "queries" => parse_queries(&args[1..]),
        "extract" => parse_extract(&args[1..]),
        other => Err(JevError::input_invalid(format!("unknown command {other}"))),
    }
}

fn parse_queries(args: &[String]) -> Result<Cli, JevError> {
    let mut language = None;
    let mut index = 0;
    while index < args.len() {
        if let Some(value) = flag_value("--language", args, &mut index)? {
            if language.is_some() {
                return Err(JevError::input_invalid("--language was given twice"));
            }
            language = Some(value);
            continue;
        }
        return Err(JevError::input_invalid(format!(
            "unexpected argument {}",
            args[index]
        )));
    }
    Ok(Cli::Queries { language })
}

fn parse_extract(args: &[String]) -> Result<Cli, JevError> {
    let mut language = None;
    let mut query = None;
    let mut captures = None;
    let mut rev = None;
    let mut path = None;
    let mut index = 0;
    while index < args.len() {
        if let Some(value) = flag_value("--language", args, &mut index)? {
            assign_once("language", &mut language, value)?;
            continue;
        }
        if let Some(value) = flag_value("--query", args, &mut index)? {
            assign_once("query", &mut query, value)?;
            continue;
        }
        if let Some(value) = flag_value("--captures", args, &mut index)? {
            assign_once("captures", &mut captures, split_captures(&value)?)?;
            continue;
        }
        if let Some(value) = flag_value("--rev", args, &mut index)? {
            assign_once("rev", &mut rev, Rev::parse(&value)?)?;
            continue;
        }
        if args[index].starts_with('-') {
            return Err(JevError::input_invalid(format!(
                "unknown flag {}",
                args[index]
            )));
        }
        if path.is_some() {
            return Err(JevError::input_invalid("extract takes one source path"));
        }
        path = Some(args[index].clone());
        index += 1;
    }
    let language =
        language.ok_or_else(|| JevError::input_invalid("extract requires --language"))?;
    let query = query.ok_or_else(|| JevError::input_invalid("extract requires --query"))?;
    let path = path.ok_or_else(|| JevError::input_invalid("extract requires a source path"))?;
    Ok(Cli::Extract {
        language,
        query,
        captures: captures.unwrap_or_default(),
        rev: rev.unwrap_or_else(|| Rev::new("HEAD")),
        path,
    })
}

fn assign_once<T>(name: &str, slot: &mut Option<T>, value: T) -> Result<(), JevError> {
    if slot.is_some() {
        return Err(JevError::input_invalid(format!("--{name} was given twice")));
    }
    *slot = Some(value);
    Ok(())
}

fn split_captures(value: &str) -> Result<Vec<String>, JevError> {
    if value.is_empty() {
        return Err(JevError::input_invalid(
            "--captures requires a comma-separated allow-list",
        ));
    }
    let mut names = Vec::new();
    for part in value.split(',') {
        let name = part.trim();
        if name.is_empty() {
            return Err(JevError::input_invalid("--captures contains an empty name"));
        }
        names.push(name.to_owned());
    }
    Ok(names)
}

/// Reads `--name value` or `--name=value` and advances `index` past it.
///
/// Returns `Ok(None)` when `args[*index]` is a different argument and leaves
/// `index` unchanged.
fn flag_value(name: &str, args: &[String], index: &mut usize) -> Result<Option<String>, JevError> {
    let arg = &args[*index];
    let with_equals = format!("{name}=");
    if let Some(value) = arg.strip_prefix(&with_equals) {
        if value.is_empty() {
            return Err(JevError::input_invalid(format!("{name} requires a value")));
        }
        *index += 1;
        return Ok(Some(value.to_owned()));
    }
    if arg == name {
        let value = args
            .get(*index + 1)
            .ok_or_else(|| JevError::input_invalid(format!("{name} requires a value")))?;
        if value.starts_with('-') {
            return Err(JevError::input_invalid(format!("{name} requires a value")));
        }
        *index += 2;
        return Ok(Some(value.clone()));
    }
    Ok(None)
}
