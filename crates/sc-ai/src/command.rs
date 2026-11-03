//! Argv parsing and `jev` command dispatch.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use sc_agent_types::observability::v2::SharedSessionLog;
use sc_agent_types::{
    BackendId, CanonicalJudgePayload, ChoiceAnswer, JevError, JudgeOutcome, PayloadBundle,
    Question, RepoRoot, Rev, SessionId, payload_bundle_from_value,
};
use sc_jev_check::config;
use sc_jev_check::registry::{BackendRegistry, registry_for_repo};
use sc_jev_check::run::{RunOptions, RunRequest, run_check, run_request};
use sc_jev_check::session::{FileSessionStore, SessionStore};
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

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
        let document = parse_request(&bytes)?;
        let backends = resolve_backends(&command.backends, &document.backends)?;
        let bundle = bundle_from_request(&document)?;
        let system_prompt = request_prompt(
            &repo,
            &backends,
            command.prompt_file.as_deref(),
            document.system_prompt.as_deref(),
        )?;
        PreparedRun::Request {
            backends,
            bundle,
            system_prompt,
        }
    } else {
        let check_path = command
            .check
            .clone()
            .unwrap_or_else(|| PathBuf::from("check.toml"));
        let relative = relative_to_repo(&repo, &check_path)?;
        let spec = config::load_check_at(&repo, &rev, &relative)?;
        let backends = resolve_backends(&command.backends, &[])?;
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
    let mut data = outcome_fields(&outcome);
    data["session_id"] = Value::String(session_id.to_string());
    data["prompt_sha256"] = Value::String(stored.payload.sha256.as_str().to_owned());
    data["readback"] = serde_json::to_value(&readback).map_err(|_| JevError::InputInvalid {
        detail: "response JSON could not be encoded".into(),
    })?;
    envelope::envelope(OP_ASK, data)
}

fn run_data(
    result: &sc_jev_check::run::CheckResult,
    readback: &RunReadback,
    dry_run: bool,
) -> Result<Value, JevError> {
    let mut data = serde_json::json!({
        "session_id": result.session_id,
        "prompt_sha256": result.bundle.sha256.as_str(),
        "readback": readback,
    });
    if dry_run {
        data["payload"] =
            serde_json::from_slice(&result.bundle.bytes).map_err(|_| JevError::InputInvalid {
                detail: "canonical payload was not JSON".into(),
            })?;
    } else {
        data["outcomes"] =
            serde_json::to_value(&result.outcomes).map_err(|_| JevError::InputInvalid {
                detail: "response JSON could not be encoded".into(),
            })?;
        data["equal_input"] =
            serde_json::to_value(&result.equal_input).map_err(|_| JevError::InputInvalid {
                detail: "response JSON could not be encoded".into(),
            })?;
        if let Some(outcome) = result.outcomes.first() {
            copy_outcome(&mut data, outcome);
        }
    }
    Ok(data)
}

fn outcome_fields(outcome: &JudgeOutcome) -> Value {
    let mut data = serde_json::json!({});
    copy_outcome(&mut data, outcome);
    data
}

fn copy_outcome(data: &mut Value, outcome: &JudgeOutcome) {
    data["answers"] = outcome.answers.clone();
    data["backend"] = serde_json::json!(outcome.backend);
    data["probabilities"] = outcome.probabilities.clone().unwrap_or(Value::Null);
    data["provider_meta"] = outcome.provider_meta.clone().unwrap_or(Value::Null);
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestDocument {
    questions: Vec<Question>,
    #[serde(default)]
    state: Vec<ChoiceAnswer>,
    #[serde(default)]
    backends: Vec<String>,
    #[serde(default)]
    system_prompt: Option<String>,
}

fn parse_request(bytes: &[u8]) -> Result<RequestDocument, JevError> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Err(JevError::InputInvalid {
            detail: "request JSON is empty. Pass a questions document on --request-json.".into(),
        });
    }
    serde_json::from_slice(bytes).map_err(|error| JevError::InputInvalid {
        detail: match error.classify() {
            serde_json::error::Category::Syntax => {
                "request JSON is not valid JSON. Pass a questions document.".into()
            }
            serde_json::error::Category::Eof => "request JSON was empty or truncated.".into(),
            serde_json::error::Category::Io => "request JSON could not be read.".into(),
            serde_json::error::Category::Data => {
                "request JSON must contain a questions array and may contain state, backends, and system_prompt.".into()
            }
        },
    })
}

fn bundle_from_request(document: &RequestDocument) -> Result<PayloadBundle, JevError> {
    if document.questions.is_empty() {
        return Err(JevError::InputInvalid {
            detail: "request JSON has no questions".into(),
        });
    }
    for answer in &document.state {
        let Some(question) = document
            .questions
            .iter()
            .find(|question| question.id == answer.question_id)
        else {
            return Err(JevError::InputInvalid {
                detail: format!(
                    "state question {} is not in the question set",
                    answer.question_id
                ),
            });
        };
        if !question
            .choices
            .iter()
            .any(|choice| choice == &answer.choice)
        {
            return Err(JevError::InputInvalid {
                detail: format!(
                    "state question {} has a choice that is not in the question",
                    answer.question_id
                ),
            });
        }
    }
    let value = serde_json::to_value(CanonicalJudgePayload {
        questions: document.questions.clone(),
        state: document.state.clone(),
    })
    .map_err(|_| JevError::InputInvalid {
        detail: "request JSON could not be encoded".into(),
    })?;
    payload_bundle_from_value(&value)
}

fn request_prompt(
    repo: &RepoRoot,
    backends: &[BackendId],
    override_path: Option<&Path>,
    json_prompt: Option<&str>,
) -> Result<Option<String>, JevError> {
    if !needs_llm(backends) {
        return Ok(None);
    }
    if let Some(path) = override_path {
        return Ok(Some(read_prompt_file(repo, path)?));
    }
    if let Some(text) = json_prompt.filter(|text| !text.trim().is_empty()) {
        return Ok(Some(text.to_owned()));
    }
    Ok(Some(config::PACKAGED_SYSTEM_PROMPT.to_owned()))
}

fn read_prompt_file(repo: &RepoRoot, path: &Path) -> Result<String, JevError> {
    let full = joined_path(repo.as_path().to_path_buf(), path);
    let display = full.display().to_string();
    match std::fs::read_to_string(&full) {
        Ok(text) if !text.trim().is_empty() => Ok(text),
        Ok(_) => Err(JevError::ConfigInvalid {
            detail: format!("system prompt {display} is empty"),
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(JevError::PromptFileMissing { path: display })
        }
        Err(error) => Err(JevError::ConfigInvalid {
            detail: format!("system prompt {display} could not be read ({error})"),
        }),
    }
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
            JevError::InputInvalid {
                detail: format!(
                    "request JSON {} could not be read ({error})",
                    path.display()
                ),
            }
        }
    })
}

fn resolve_backends(
    flags: &[String],
    json_backends: &[String],
) -> Result<Vec<BackendId>, JevError> {
    let source: &[String] = if !flags.is_empty() {
        flags
    } else if !json_backends.is_empty() {
        json_backends
    } else {
        return Ok(vec![BackendId::Jev]);
    };
    source.iter().map(|text| parse_backend(text)).collect()
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

fn needs_llm(backends: &[BackendId]) -> bool {
    backends
        .iter()
        .any(|id| matches!(id, BackendId::AnthropicHaiku | BackendId::OpenAiLuna))
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
    let cwd = std::env::current_dir().map_err(|error| JevError::ConfigInvalid {
        detail: format!("current directory is unavailable ({error}). Pass --repo-root."),
    })?;
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&cwd)
        .output()
        .map_err(|error| JevError::ConfigInvalid {
            detail: format!("git rev-parse failed ({error}). Pass --repo-root."),
        })?;
    if !output.status.success() {
        return Err(JevError::ConfigInvalid {
            detail: "Could not find a git toplevel. Pass --repo-root with the repository root."
                .into(),
        });
    }
    let text = String::from_utf8(output.stdout).map_err(|_| JevError::ConfigInvalid {
        detail: "git rev-parse returned non-UTF-8. Pass --repo-root.".into(),
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
