//! Thin CLI over [`sc_treesitter`].
//!
//! Success writes JSON to stdout. Failure writes one `JevError` object to
//! stderr and exits 2.

use std::io::{self, Write};
use std::process::ExitCode;

use mimalloc::MiMalloc;
use sc_agent_types::{JevError, ScAgentErrorResponse};
use sc_treesitter::{
    ExtractInput, extract, languages, queries, query_source, subjects_to_json_bytes,
};

mod git_io;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

const HELP: &str = "\
sc-treesitter extract --language L --query Q [--captures name,name] [--rev HEAD] <path>
sc-treesitter languages
sc-treesitter queries [--language L]

<path> is repo-relative. --rev HEAD reads `git show`. --rev WORKTREE reads the
working tree. --captures is an allow-list. Execution always uses matches mode.
";

fn main() -> ExitCode {
    match dispatch(std::env::args().skip(1)) {
        Ok(bytes) => {
            if let Err(error) = io::stdout().write_all(&bytes) {
                emit(&JevError::ExtractionFailed {
                    detail: format!("stdout write failed: {error}"),
                });
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
            &JevError::ExtractionFailed {
                detail: "stderr encoding".to_owned(),
            },
        ))
        .expect("ScAgentErrorResponse from JevError must serialize")
    });
    let _ = writeln!(io::stderr(), "{line}");
}

fn dispatch(args: impl Iterator<Item = String>) -> Result<Vec<u8>, JevError> {
    match parse(args)? {
        Cli::Help => Ok(HELP.as_bytes().to_vec()),
        Cli::Languages => {
            let ids: Vec<&str> = languages().iter().map(|info| info.id).collect();
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
            let document = extract(&ExtractInput {
                language: &language,
                query: query_text,
                captures: allow,
                source: &blob.bytes,
                path: &blob.path,
                rev: &rev,
                blob_sha: &blob.sha,
            })?;
            subjects_to_json_bytes(&document)
        }
    }
}

fn json_line(value: &serde_json::Value) -> Result<Vec<u8>, JevError> {
    let mut bytes = serde_json::to_vec(value).map_err(|error| JevError::ExtractionFailed {
        detail: format!("json encoding failed: {error}"),
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

enum Cli {
    Help,
    Languages,
    Queries {
        language: Option<String>,
    },
    Extract {
        language: String,
        query: String,
        captures: Vec<String>,
        rev: String,
        path: String,
    },
}

fn parse(args: impl Iterator<Item = String>) -> Result<Cli, JevError> {
    let args: Vec<String> = args.collect();
    if args.is_empty() || args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(Cli::Help);
    }
    match args[0].as_str() {
        "help" => Ok(Cli::Help),
        "languages" => {
            if args.len() != 1 {
                return Err(JevError::InputInvalid {
                    detail: "languages does not take arguments".into(),
                });
            }
            Ok(Cli::Languages)
        }
        "queries" => parse_queries(&args[1..]),
        "extract" => parse_extract(&args[1..]),
        other => Err(JevError::InputInvalid {
            detail: format!("unknown command {other}"),
        }),
    }
}

fn parse_queries(args: &[String]) -> Result<Cli, JevError> {
    let mut language = None;
    let mut index = 0;
    while index < args.len() {
        if let Some(value) = flag_value("--language", args, &mut index)? {
            if language.is_some() {
                return Err(JevError::InputInvalid {
                    detail: "--language was given twice".into(),
                });
            }
            language = Some(value);
            continue;
        }
        return Err(JevError::InputInvalid {
            detail: format!("unexpected argument {}", args[index]),
        });
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
            assign_once("rev", &mut rev, value)?;
            continue;
        }
        if args[index].starts_with('-') {
            return Err(JevError::InputInvalid {
                detail: format!("unknown flag {}", args[index]),
            });
        }
        if path.is_some() {
            return Err(JevError::InputInvalid {
                detail: "extract takes one source path".into(),
            });
        }
        path = Some(args[index].clone());
        index += 1;
    }
    let language = language.ok_or_else(|| JevError::InputInvalid {
        detail: "extract requires --language".into(),
    })?;
    let query = query.ok_or_else(|| JevError::InputInvalid {
        detail: "extract requires --query".into(),
    })?;
    let path = path.ok_or_else(|| JevError::InputMissing {
        path: "(missing)".into(),
    })?;
    Ok(Cli::Extract {
        language,
        query,
        captures: captures.unwrap_or_default(),
        rev: rev.unwrap_or_else(|| "HEAD".to_owned()),
        path,
    })
}

fn assign_once<T>(name: &str, slot: &mut Option<T>, value: T) -> Result<(), JevError> {
    if slot.is_some() {
        return Err(JevError::InputInvalid {
            detail: format!("--{name} was given twice"),
        });
    }
    *slot = Some(value);
    Ok(())
}

fn split_captures(value: &str) -> Result<Vec<String>, JevError> {
    if value.is_empty() {
        return Err(JevError::InputInvalid {
            detail: "--captures requires a comma-separated allow-list".into(),
        });
    }
    let mut names = Vec::new();
    for part in value.split(',') {
        let name = part.trim();
        if name.is_empty() {
            return Err(JevError::InputInvalid {
                detail: "--captures contains an empty name".into(),
            });
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
            return Err(JevError::InputInvalid {
                detail: format!("{name} requires a value"),
            });
        }
        *index += 1;
        return Ok(Some(value.to_owned()));
    }
    if arg == name {
        let value = args.get(*index + 1).ok_or_else(|| JevError::InputInvalid {
            detail: format!("{name} requires a value"),
        })?;
        if value.starts_with('-') {
            return Err(JevError::InputInvalid {
                detail: format!("{name} requires a value"),
            });
        }
        *index += 2;
        return Ok(Some(value.clone()));
    }
    Ok(None)
}
