//! Argv parsing and `jev` command dispatch.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use sc_agent_types::observability::v2::SharedSessionLog;
use sc_agent_types::{BackendId, JevError, JudgeOutcome, PayloadBundle, RepoRoot, Rev, SessionId};
use sc_jev_check::config;
use sc_jev_check::registry::{BackendRegistry, registry_for_repo};
use sc_jev_check::run::{RunOptions, RunRequest, prepare_caller_request, run_check, run_request};
use sc_jev_check::session::{FileSessionStore, SessionStore};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::envelope::{self, OP_ASK, OP_DRY_RUN, OP_RUN};
use crate::observe::{
    ObservabilitySessionLog, default_log_root, default_sessions_dir as sessions_dir_for,
};

/// Clock injected into the session store.
pub type Clock = Arc<dyn Fn() -> SystemTime + Send + Sync>;

/// Test and embedding overrides for [`execute`].
///
/// Production passes [`Self::default`]. A registry skips key resolution and
/// HTTP construction inside `sc-jev-check`. A session log replaces the v2
/// file logger.
#[derive(Default)]
pub struct ExecuteOverrides {
    /// Backends used instead of `registry_for_repo`.
    pub registry: Option<BackendRegistry>,
    /// Clock for session `created_at` and TTL checks.
    pub clock: Option<Clock>,
    /// Session lifetime measured from `created_at`.
    pub session_ttl: Option<Duration>,
    /// Logger used instead of [`ObservabilitySessionLog`].
    pub session_log: Option<SharedSessionLog>,
}

/// Reports whether `args` need stdin for `--request-json -`.
#[must_use]
pub fn reads_stdin(args: &[String]) -> bool {
    args.windows(2)
        .any(|pair| pair[0] == "--request-json" && pair[1] == "-")
        || args.iter().any(|arg| arg == "--request-json=-")
}

/// Runs one `sc-ai` invocation.
///
/// `args` excludes the program name. `stdin` is used only when
/// [`reads_stdin`] is true. Success bytes are the versioned envelope, help,
/// or the version line. Failures are [`JevError`] values; the binary prints
/// them as stderr JSON and exits 2.
///
/// # Errors
///
/// Returns [`JevError`] for unknown argv, unreadable paths, config, input,
/// key, session, prompt, or judge failures. A dry-run does not resolve keys.
pub fn execute(
    args: &[String],
    stdin: &[u8],
    overrides: ExecuteOverrides,
) -> Result<Vec<u8>, JevError> {
    if args.is_empty() || args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(text_bytes(help_for(args)));
    }
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        return Ok(text_bytes(&version_line()));
    }
    match parse(args, stdin)? {
        Parsed::Run(run) => dispatch_run(run, false, &overrides),
        Parsed::DryRun(run) => dispatch_run(run, true, &overrides),
        Parsed::Ask(ask) => dispatch_ask(ask, &overrides),
    }
}

fn version_line() -> String {
    format!("sc-ai {}", env!("CARGO_PKG_VERSION"))
}

fn text_bytes(text: &str) -> Vec<u8> {
    let mut bytes = text.as_bytes().to_vec();
    if !bytes.ends_with(b"\n") {
        bytes.push(b'\n');
    }
    bytes
}

fn help_for(args: &[String]) -> &'static str {
    if args.iter().any(|arg| arg == "jev") {
        JEV_HELP
    } else {
        GLOBAL_HELP
    }
}

const GLOBAL_HELP: &str = "\
sc-ai jev run|dry-run|ask [flags]
sc-ai --version

Success writes one JSON envelope to stdout: {\"version\",\"operation\",\"data\"}.
Failure writes {\"error\",\"message\"} to stderr and exits 2.

Pass sc-ai jev --help for check, repo, prompt, and log flags.
";

const JEV_HELP: &str = "\
sc-ai jev run [--repo-root PATH] [--check PATH] [--rev REV]
    [--backend ID]... [--request-json PATH|-]
    [--llm-system-prompt-file PATH] [--sessions-dir PATH] [--log-dir PATH]
sc-ai jev dry-run [same flags as run]
sc-ai jev ask --session-id UUID --text TEXT
    [--repo-root PATH] [--sessions-dir PATH] [--log-dir PATH]

--check is a repo-relative check.toml (default check.toml).
--request-json is a caller-built questions document, or - for stdin.
--backend is jev, anthropic_haiku, or openai_luna. The default is jev.
--llm-system-prompt-file overrides the LLM system prompt. It changes the
provider wire, not prompt_sha256.
--rev defaults to WORKTREE.

Default log root: <repo-root>/.sc-ai
Override with --log-dir or SC_LOG_ROOT.
Session JSONL: <log-root>/logs/sc-ai.log.jsonl
Session files: <repo-root>/.sc-ai/sessions (override with --sessions-dir).

data includes the Jev answer fields and session_id. Dry-run returns
prompt_sha256 and does not resolve API keys.
";

struct RunCommand {
    repo_root: Option<PathBuf>,
    check: Option<PathBuf>,
    rev: Option<String>,
    backends: Vec<String>,
    request_json: Option<String>,
    prompt_file: Option<PathBuf>,
    sessions_dir: Option<PathBuf>,
    log_dir: Option<PathBuf>,
    stdin: Vec<u8>,
}

struct AskCommand {
    repo_root: Option<PathBuf>,
    session_id: String,
    text: String,
    sessions_dir: Option<PathBuf>,
    log_dir: Option<PathBuf>,
}

enum Parsed {
    Run(RunCommand),
    DryRun(RunCommand),
    Ask(AskCommand),
}

#[derive(Default)]
struct Flags {
    repo_root: Option<PathBuf>,
    check: Option<PathBuf>,
    rev: Option<String>,
    backends: Vec<String>,
    request_json: Option<String>,
    prompt_file: Option<PathBuf>,
    sessions_dir: Option<PathBuf>,
    log_dir: Option<PathBuf>,
    session_id: Option<String>,
    text: Option<String>,
}

fn parse(args: &[String], stdin: &[u8]) -> Result<Parsed, JevError> {
    let Some(head) = args.first() else {
        return Err(unknown_command(""));
    };
    if head != "jev" {
        return Err(unknown_command(head));
    }
    let Some(sub) = args.get(1) else {
        return Err(JevError::InputInvalid {
            detail: "jev requires run, dry-run, or ask. See sc-ai jev --help.".into(),
        });
    };
    let flags = parse_flags(&args[2..])?;
    match sub.as_str() {
        "run" | "dry-run" => {
            reject_ask_flags(sub, &flags)?;
            if flags.request_json.is_some() && flags.check.is_some() {
                return Err(JevError::InputInvalid {
                    detail: "pass either --request-json or --check, not both".into(),
                });
            }
            let command = RunCommand {
                repo_root: flags.repo_root,
                check: flags.check,
                rev: flags.rev,
                backends: flags.backends,
                request_json: flags.request_json,
                prompt_file: flags.prompt_file,
                sessions_dir: flags.sessions_dir,
                log_dir: flags.log_dir,
                stdin: stdin.to_vec(),
            };
            if sub == "dry-run" {
                Ok(Parsed::DryRun(command))
            } else {
                Ok(Parsed::Run(command))
            }
        }
        "ask" => {
            reject_run_flags(&flags)?;
            let session_id = flags.session_id.ok_or_else(|| JevError::InputInvalid {
                detail: "ask requires --session-id. See sc-ai jev --help.".into(),
            })?;
            let text = flags.text.ok_or_else(|| JevError::InputInvalid {
                detail: "ask requires --text. See sc-ai jev --help.".into(),
            })?;
            Ok(Parsed::Ask(AskCommand {
                repo_root: flags.repo_root,
                session_id,
                text,
                sessions_dir: flags.sessions_dir,
                log_dir: flags.log_dir,
            }))
        }
        other => Err(JevError::InputInvalid {
            detail: format!("unknown jev command {other}. See sc-ai jev --help."),
        }),
    }
}

fn reject_ask_flags(command: &str, flags: &Flags) -> Result<(), JevError> {
    if flags.session_id.is_some() || flags.text.is_some() {
        return Err(JevError::InputInvalid {
            detail: format!("{command} does not take --session-id or --text"),
        });
    }
    Ok(())
}

fn reject_run_flags(flags: &Flags) -> Result<(), JevError> {
    if flags.check.is_some()
        || flags.request_json.is_some()
        || flags.prompt_file.is_some()
        || flags.rev.is_some()
        || !flags.backends.is_empty()
    {
        return Err(JevError::InputInvalid {
            detail: "ask does not take --check, --request-json, --backend, --rev, or --llm-system-prompt-file".into(),
        });
    }
    Ok(())
}

fn parse_flags(args: &[String]) -> Result<Flags, JevError> {
    let mut flags = Flags::default();
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].clone();
        index += 1;
        if let Some((name, value)) = arg
            .split_once('=')
            .filter(|(name, _)| name.starts_with("--"))
        {
            assign(&mut flags, name, value.to_owned())?;
            continue;
        }
        match arg.as_str() {
            "--repo-root" => {
                flags.repo_root = Some(PathBuf::from(next_value(args, &mut index, &arg, false)?))
            }
            "--check" => {
                flags.check = Some(PathBuf::from(next_value(args, &mut index, &arg, false)?))
            }
            "--rev" => flags.rev = Some(next_value(args, &mut index, &arg, false)?),
            "--backend" => flags
                .backends
                .push(next_value(args, &mut index, &arg, false)?),
            "--request-json" => {
                flags.request_json = Some(next_value(args, &mut index, &arg, true)?);
            }
            "--llm-system-prompt-file" => {
                flags.prompt_file = Some(PathBuf::from(next_value(args, &mut index, &arg, false)?));
            }
            "--sessions-dir" => {
                flags.sessions_dir =
                    Some(PathBuf::from(next_value(args, &mut index, &arg, false)?));
            }
            "--log-dir" => {
                flags.log_dir = Some(PathBuf::from(next_value(args, &mut index, &arg, false)?))
            }
            "--session-id" => flags.session_id = Some(next_value(args, &mut index, &arg, false)?),
            "--text" => flags.text = Some(next_value(args, &mut index, &arg, false)?),
            other if other.starts_with('-') => {
                return Err(JevError::InputInvalid {
                    detail: format!("unknown flag {other}. See sc-ai jev --help."),
                });
            }
            other => {
                return Err(JevError::InputInvalid {
                    detail: format!("unexpected argument {other}. See sc-ai jev --help."),
                });
            }
        }
    }
    Ok(flags)
}

fn assign(flags: &mut Flags, flag: &str, value: String) -> Result<(), JevError> {
    if value.is_empty() || (value.starts_with('-') && !(flag == "--request-json" && value == "-")) {
        return Err(missing_value(flag));
    }
    match flag {
        "--repo-root" => flags.repo_root = Some(PathBuf::from(value)),
        "--check" => flags.check = Some(PathBuf::from(value)),
        "--rev" => flags.rev = Some(value),
        "--backend" => flags.backends.push(value),
        "--request-json" => flags.request_json = Some(value),
        "--llm-system-prompt-file" => flags.prompt_file = Some(PathBuf::from(value)),
        "--sessions-dir" => flags.sessions_dir = Some(PathBuf::from(value)),
        "--log-dir" => flags.log_dir = Some(PathBuf::from(value)),
        "--session-id" => flags.session_id = Some(value),
        "--text" => flags.text = Some(value),
        other => {
            return Err(JevError::InputInvalid {
                detail: format!("unknown flag {other}. See sc-ai jev --help."),
            });
        }
    }
    Ok(())
}

fn next_value(
    args: &[String],
    index: &mut usize,
    flag: &str,
    allow_dash: bool,
) -> Result<String, JevError> {
    let Some(value) = args.get(*index) else {
        return Err(missing_value(flag));
    };
    if value.starts_with('-') && !(allow_dash && value == "-") {
        return Err(missing_value(flag));
    }
    *index += 1;
    Ok(value.clone())
}

fn missing_value(flag: &str) -> JevError {
    JevError::InputInvalid {
        detail: format!("{flag} requires a value. See sc-ai jev --help."),
    }
}

fn unknown_command(name: &str) -> JevError {
    JevError::InputInvalid {
        detail: format!("unknown command {name}. See sc-ai --help."),
    }
}

struct PreparedLog {
    shared: SharedSessionLog,
    adapter: Option<Arc<ObservabilitySessionLog>>,
}

fn prepare_log(log_root: &Path, overrides: &ExecuteOverrides) -> Result<PreparedLog, JevError> {
    if let Some(shared) = overrides.session_log.clone() {
        return Ok(PreparedLog {
            shared,
            adapter: None,
        });
    }
    let adapter = ObservabilitySessionLog::open(log_root)?;
    let shared: SharedSessionLog = adapter.clone();
    Ok(PreparedLog {
        shared,
        adapter: Some(adapter),
    })
}

fn finish_log(prepared: &PreparedLog) -> Result<(), JevError> {
    if let Some(adapter) = &prepared.adapter {
        adapter.flush()?;
    }
    Ok(())
}

fn open_sessions(
    dir: &Path,
    log: SharedSessionLog,
    overrides: &ExecuteOverrides,
) -> Result<FileSessionStore, JevError> {
    let mut sessions = FileSessionStore::new(dir, log)?;
    if let Some(ttl) = overrides.session_ttl {
        sessions = sessions.with_ttl(ttl);
    }
    if let Some(clock) = overrides.clock.clone() {
        sessions = sessions.with_clock(move || clock());
    }
    Ok(sessions)
}

fn dispatch_run(
    command: RunCommand,
    dry_run: bool,
    overrides: &ExecuteOverrides,
) -> Result<Vec<u8>, JevError> {
    let repo = resolve_repo(command.repo_root.as_deref())?;
    let rev = command.rev.as_deref().map_or_else(Rev::worktree, Rev::new);
    let sessions_dir = command
        .sessions_dir
        .clone()
        .unwrap_or_else(|| sessions_dir_for(&repo));
    let log_root = default_log_root(&repo, command.log_dir.as_deref());
    let prompt_path = command
        .prompt_file
        .as_ref()
        .map(|path| joined_path(repo.as_path().to_path_buf(), path));
    let plan = if let Some(request_path) = &command.request_json {
        let bytes = request_bytes(&repo, request_path, &command.stdin)?;
        let prepared = prepare_caller_request(
            &bytes,
            &parse_flag_backends(&command.backends)?,
            &repo,
            command.prompt_file.as_deref(),
        )?;
        PreparedRun::Request {
            backends: prepared.backends,
            bundle: prepared.bundle,
            system_prompt: prepared.system_prompt,
        }
    } else {
        let check_path = command
            .check
            .clone()
            .unwrap_or_else(|| PathBuf::from("check.toml"));
        let relative = relative_to_repo(&repo, &check_path)?;
        let spec = config::load_check_at(&repo, &rev, &relative)?;
        let backends = if command.backends.is_empty() {
            vec![BackendId::Jev]
        } else {
            parse_flag_backends(&command.backends)?
        };
        PreparedRun::Check { spec, backends }
    };
    let prepared = prepare_log(&log_root, overrides)?;
    let sessions = open_sessions(&sessions_dir, Arc::clone(&prepared.shared), overrides)?;
    let readback = RunReadback {
        check_toml: command
            .check
            .as_ref()
            .map(|path| absolute_text(&repo, path))
            .or_else(|| {
                if command.request_json.is_none() {
                    Some(absolute_text(&repo, Path::new("check.toml")))
                } else {
                    None
                }
            }),
        repo_root: path_text(repo.as_path()),
        llm_system_prompt_file: prompt_path.as_ref().map(|path| path_text(path)),
        request_json: command.request_json.as_ref().map(|path| {
            if path == "-" {
                "-".to_owned()
            } else {
                absolute_text(&repo, Path::new(path))
            }
        }),
        sessions_dir: path_text(&sessions_dir),
    };
    let mut opts = RunOptions::new(repo, sessions, Arc::clone(&prepared.shared))
        .dry_run(dry_run)
        .rev(rev);
    if let Some(registry) = &overrides.registry {
        opts = opts.registry(registry.clone());
    }
    let result = match plan {
        PreparedRun::Request {
            backends,
            bundle,
            system_prompt,
        } => run_request(
            &RunRequest {
                bundle,
                system_prompt,
                producer: None,
            },
            &backends,
            opts,
        ),
        PreparedRun::Check { spec, backends } => {
            if let Some(path) = prompt_path {
                opts = opts.system_prompt_override(path);
            }
            run_check(&spec, &backends, opts)
        }
    }?;
    finish_log(&prepared)?;
    let operation = if dry_run { OP_DRY_RUN } else { OP_RUN };
    envelope::envelope(operation, run_data(&result, &readback, dry_run)?)
}

enum PreparedRun {
    Request {
        backends: Vec<BackendId>,
        bundle: PayloadBundle,
        system_prompt: Option<String>,
    },
    Check {
        spec: config::CheckSpec,
        backends: Vec<BackendId>,
    },
}

fn joined_path(repo: PathBuf, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo.join(path)
    }
}

fn dispatch_ask(command: AskCommand, overrides: &ExecuteOverrides) -> Result<Vec<u8>, JevError> {
    let repo = resolve_repo(command.repo_root.as_deref())?;
    let session_id = parse_session_id(&command.session_id)?;
    let sessions_dir = command
        .sessions_dir
        .clone()
        .unwrap_or_else(|| sessions_dir_for(&repo));
    let log_root = default_log_root(&repo, command.log_dir.as_deref());
    let prepared = prepare_log(&log_root, overrides)?;
    let sessions = open_sessions(&sessions_dir, Arc::clone(&prepared.shared), overrides)?;
    let stored = sessions.load(&session_id)?;
    if let Some(registry) = &overrides.registry {
        sessions.install_registry(registry.clone());
    } else {
        sessions.install_registry(registry_for_repo(&repo, &[stored.backend])?);
    }
    let outcome = sessions.ask(&session_id, &command.text)?;
    finish_log(&prepared)?;
    let readback = AskReadback {
        repo_root: path_text(repo.as_path()),
        sessions_dir: path_text(&sessions_dir),
        session_id: session_id.to_string(),
    };
    let mut body = Map::new();
    merge_jev_response(&mut body, &outcome);
    body.insert(
        "session_id".to_owned(),
        Value::String(session_id.to_string()),
    );
    body.insert(
        "prompt_sha256".to_owned(),
        Value::String(stored.payload.sha256.as_str().to_owned()),
    );
    body.insert(
        "readback".to_owned(),
        serde_json::to_value(&readback).map_err(|_| JevError::InputInvalid {
            detail: "response JSON could not be encoded".into(),
        })?,
    );
    envelope::envelope(OP_ASK, Value::Object(body))
}

fn run_data(
    result: &sc_jev_check::run::CheckResult,
    readback: &RunReadback,
    dry_run: bool,
) -> Result<Value, JevError> {
    let mut data = Map::new();
    data.insert(
        "session_id".to_owned(),
        serde_json::to_value(result.session_id).map_err(|_| JevError::InputInvalid {
            detail: "response JSON could not be encoded".into(),
        })?,
    );
    data.insert(
        "prompt_sha256".to_owned(),
        Value::String(result.bundle.sha256.as_str().to_owned()),
    );
    data.insert(
        "readback".to_owned(),
        serde_json::to_value(readback).map_err(|_| JevError::InputInvalid {
            detail: "response JSON could not be encoded".into(),
        })?,
    );
    if dry_run {
        data.insert(
            "payload".to_owned(),
            serde_json::from_slice(&result.bundle.bytes).map_err(|_| JevError::InputInvalid {
                detail: "canonical payload was not JSON".into(),
            })?,
        );
    } else {
        data.insert(
            "outcomes".to_owned(),
            serde_json::to_value(&result.outcomes).map_err(|_| JevError::InputInvalid {
                detail: "response JSON could not be encoded".into(),
            })?,
        );
        data.insert(
            "equal_input".to_owned(),
            serde_json::to_value(&result.equal_input).map_err(|_| JevError::InputInvalid {
                detail: "response JSON could not be encoded".into(),
            })?,
        );
        if let Some(outcome) = result.outcomes.first() {
            merge_jev_response(&mut data, outcome);
        }
    }
    Ok(Value::Object(data))
}

/// Copies the Jev response fields into `data` and leaves `model` and `usage`
/// unchanged (ADR-SAG-0014).
///
/// `answers` is the validated answer object. `model` and `usage` are copied
/// from provider metadata only when the judge stored them under those names.
/// They are not nested under `provider_meta` and their values are not rebuilt.
/// LLM outcomes use the same fields: normalized `answers`, plus `model` or
/// `usage` when that backend recorded them.
fn merge_jev_response(data: &mut Map<String, Value>, outcome: &JudgeOutcome) {
    data.insert("answers".to_owned(), outcome.answers.clone());
    let Some(meta) = outcome.provider_meta.as_ref().and_then(Value::as_object) else {
        return;
    };
    if let Some(model) = meta.get("model") {
        data.insert("model".to_owned(), model.clone());
    }
    if let Some(usage) = meta.get("usage") {
        data.insert("usage".to_owned(), usage.clone());
    }
}

#[derive(Serialize)]
struct RunReadback {
    check_toml: Option<String>,
    repo_root: String,
    llm_system_prompt_file: Option<String>,
    request_json: Option<String>,
    sessions_dir: String,
}

#[derive(Serialize)]
struct AskReadback {
    repo_root: String,
    sessions_dir: String,
    session_id: String,
}

fn request_bytes(repo: &RepoRoot, spec: &str, stdin: &[u8]) -> Result<Vec<u8>, JevError> {
    if spec == "-" {
        return Ok(stdin.to_vec());
    }
    let path = joined_path(repo.as_path().to_path_buf(), Path::new(spec));
    std::fs::read(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            JevError::InputMissing {
                path: path.display().to_string(),
            }
        } else {
            JevError::operational_io(&format!(
                "request JSON {} could not be read ({error}). Check the file permissions and retry.",
                path.display()
            ))
        }
    })
}

fn parse_flag_backends(flags: &[String]) -> Result<Vec<BackendId>, JevError> {
    flags.iter().map(|text| parse_backend(text)).collect()
}

fn parse_backend(text: &str) -> Result<BackendId, JevError> {
    serde_json::from_value(Value::String(text.to_owned())).map_err(|_| JevError::InputInvalid {
        detail: format!("backend {text} is not jev, anthropic_haiku, or openai_luna"),
    })
}

fn parse_session_id(text: &str) -> Result<SessionId, JevError> {
    let id = uuid::Uuid::parse_str(text).map_err(|_| JevError::InputInvalid {
        detail: format!("session id {text} is not a UUID. Pass the id from a jev run envelope."),
    })?;
    Ok(SessionId::from_uuid(id))
}

fn resolve_repo(flag: Option<&Path>) -> Result<RepoRoot, JevError> {
    if let Some(path) = flag {
        if !path.is_dir() {
            return Err(JevError::InputMissing {
                path: path.display().to_string(),
            });
        }
        return Ok(RepoRoot::new(path));
    }
    let cwd = std::env::current_dir().map_err(|error| {
        JevError::operational_config(&format!(
            "current directory is unavailable ({error}). Pass --repo-root."
        ))
    })?;
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&cwd)
        .output()
        .map_err(|error| {
            JevError::operational_config(&format!(
                "git rev-parse failed ({error}). Pass --repo-root."
            ))
        })?;
    if !output.status.success() {
        return Err(JevError::operational_config(
            "Could not find a git toplevel. Pass --repo-root with the repository root.",
        ));
    }
    let text = String::from_utf8(output.stdout).map_err(|_| {
        JevError::operational_config("git rev-parse returned non-UTF-8. Pass --repo-root.")
    })?;
    let path = PathBuf::from(text.trim());
    if !path.is_dir() {
        return Err(JevError::InputMissing {
            path: path.display().to_string(),
        });
    }
    Ok(RepoRoot::new(path))
}

fn relative_to_repo(repo: &RepoRoot, path: &Path) -> Result<String, JevError> {
    let full = joined_path(repo.as_path().to_path_buf(), path);
    let root = std::fs::canonicalize(repo.as_path()).map_err(|_| JevError::InputMissing {
        path: repo.as_path().display().to_string(),
    })?;
    let full = std::fs::canonicalize(&full).map_err(|_| JevError::InputMissing {
        path: full.display().to_string(),
    })?;
    let rel = full
        .strip_prefix(&root)
        .map_err(|_| JevError::InputInvalid {
            detail: format!(
                "{} is outside the repo root {}. Pass a path inside --repo-root.",
                full.display(),
                root.display()
            ),
        })?;
    let text = rel.to_string_lossy().replace('\\', "/");
    if text.is_empty() {
        return Err(JevError::InputInvalid {
            detail: "check path must be a file inside the repo, not the repo root itself".into(),
        });
    }
    Ok(text)
}

fn absolute_text(repo: &RepoRoot, path: &Path) -> String {
    path_text(&joined_path(repo.as_path().to_path_buf(), path))
}

fn path_text(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_stdin_only_for_request_json_dash() {
        let args = ["jev", "run", "--request-json", "-"]
            .map(str::to_owned)
            .to_vec();
        assert!(reads_stdin(&args));
        let equals = ["jev", "run", "--request-json=-"]
            .map(str::to_owned)
            .to_vec();
        assert!(reads_stdin(&equals));
        let file = ["jev", "run", "--request-json", "req.json"]
            .map(str::to_owned)
            .to_vec();
        assert!(!reads_stdin(&file));
    }
}
