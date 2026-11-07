//! `build_request`, `run_check`, and `run_request` (REQ-SAG-0601, REQ-SAG-0603).
//!
//! [`build_request`] is the public payload assembly entry (ADR-SAG-0005).
//! [`run_check`] uses that same assembly, resolves one LLM system prompt, and
//! judges every requested backend with those bytes. Dry-run returns the digest
//! and does not resolve keys or send. Judge sends emit one [`LogEvent`]
//! through the injected [`SharedSessionLog`].

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod prompt;

use std::path::{Path, PathBuf};

use sc_agent_types::observability::v2::{LogEvent, SharedSessionLog};
use sc_agent_types::{
    BackendId, EqualInputRecord, JevError, JudgeOutcome, JudgeRequest, LlmWireContext,
    PayloadBundle, RepoRoot, Rev, SessionId, SubjectsProducer,
};

use crate::config::CheckSpec;
use crate::equal_input::{commands_for, producer_from_extractions, record, same_bytes};
use crate::payload::PayloadBuilder;
use crate::registry::{BackendRegistry, registry_for_repo};
use crate::session::{FileSessionStore, emit_log, is_llm, render_session_context};

/// Inputs for one check run.
///
/// `session_log` receives one event per judge send. Pass the same
/// [`SharedSessionLog`] to [`FileSessionStore`] so `ask` uses it too.
pub struct RunOptions {
    /// Git toplevel used for payload paths and `resolve_key`.
    pub repo_root: RepoRoot,
    /// When true, return the payload digest and do not send.
    pub dry_run: bool,
    /// Working-tree prompt file. Wins over `check.toml` and the packaged asset.
    pub system_prompt_override: Option<PathBuf>,
    /// Session log for judge sends. Best effort; rejection does not fail the run.
    pub session_log: SharedSessionLog,
    /// Revision for check inputs and the `check.toml` prompt file.
    pub rev: Rev,
    /// Session files created for a live run.
    pub sessions: FileSessionStore,
    /// Injected backends. `None` resolves keys and constructs the HTTP backends.
    pub registry: Option<BackendRegistry>,
}

impl RunOptions {
    /// Options for a live run at `WORKTREE` without an override or injected registry.
    #[must_use]
    pub fn new(
        repo_root: RepoRoot,
        sessions: FileSessionStore,
        session_log: SharedSessionLog,
    ) -> Self {
        Self {
            repo_root,
            dry_run: false,
            system_prompt_override: None,
            session_log,
            rev: Rev::worktree(),
            sessions,
            registry: None,
        }
    }

    /// Sets [`Self::dry_run`].
    #[must_use]
    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Sets the git revision used for check inputs.
    #[must_use]
    pub fn rev(mut self, rev: Rev) -> Self {
        self.rev = rev;
        self
    }

    /// Sets [`Self::system_prompt_override`].
    #[must_use]
    pub fn system_prompt_override(mut self, path: impl Into<PathBuf>) -> Self {
        self.system_prompt_override = Some(path.into());
        self
    }

    /// Uses `registry` instead of resolving keys.
    #[must_use]
    pub fn registry(mut self, registry: BackendRegistry) -> Self {
        self.registry = Some(registry);
        self
    }
}

impl std::fmt::Debug for RunOptions {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RunOptions")
            .field("repo_root", &self.repo_root)
            .field("dry_run", &self.dry_run)
            .field("system_prompt_override", &self.system_prompt_override)
            .field("rev", &self.rev)
            .finish_non_exhaustive()
    }
}

/// Outcome of [`run_check`] or [`run_request`].
#[derive(Debug, Clone, PartialEq)]
pub struct CheckResult {
    /// Session created for a live run. `None` on dry-run.
    pub session_id: Option<SessionId>,
    /// Canonical payload. Its digest is `prompt_sha256`.
    pub bundle: PayloadBundle,
    /// Base system prompt injected into LLM backends. Not part of the digest.
    pub system_prompt: Option<String>,
    /// One outcome per backend, in request order. Empty on dry-run.
    pub outcomes: Vec<JudgeOutcome>,
    /// Equal-input attestation. `None` on dry-run.
    pub equal_input: Option<EqualInputRecord>,
}

/// Canonical payload for one check file, with no judge send.
///
/// [`build_request`] and [`run_check`] both produce this assembly.
/// [`BuiltRequest::bundle`]'s digest is `prompt_sha256`. The system prompt is
/// not included (ADR-SAG-0019).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltRequest {
    /// Canonical bytes shared by every backend.
    pub bundle: PayloadBundle,
    /// Subjects producer from the first `subjects` or `command` extraction.
    pub producer: Option<SubjectsProducer>,
}

/// Loads repo-relative `check_path` and assembles one canonical payload.
///
/// This is the public assembly entry (ADR-SAG-0005). Extractors and
/// canonicalization run in this crate. Composition roots call this function
/// and do not call [`PayloadBuilder`].
///
/// # Errors
///
/// Returns [`JevError`] when `check_path` is missing or invalid, an extractor
/// fails, or the payload exceeds the packaged byte cap.
///
/// # Examples
///
/// ```no_run
/// use sc_agent_types::{RepoRoot, Rev};
/// use sc_jev_check::run::build_request;
///
/// let repo = RepoRoot::new(".");
/// let built = build_request(&repo, &Rev::worktree(), "check.toml")?;
/// let _digest = built.bundle.sha256();
/// # Ok::<(), sc_agent_types::JevError>(())
/// ```
pub fn build_request(
    repo: &RepoRoot,
    rev: &Rev,
    check_path: &str,
) -> Result<BuiltRequest, JevError> {
    let spec = crate::config::load_check_at(repo, rev, check_path)?;
    assemble_payload(&spec, repo, rev)
}

fn assemble_payload(
    spec: &CheckSpec,
    repo: &RepoRoot,
    rev: &Rev,
) -> Result<BuiltRequest, JevError> {
    let built = PayloadBuilder::packaged()?.build(spec, repo, rev)?;
    Ok(BuiltRequest {
        bundle: built.bundle,
        producer: producer_from_extractions(&built.extractions),
    })
}

/// A payload that is already built.
///
/// [`run_check`] fills this from `check.toml`. Callers that already have
/// canonical bytes use [`run_request`] directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRequest {
    /// Canonical bytes shared by every backend.
    pub bundle: PayloadBundle,
    /// Base LLM system prompt. Required when an LLM backend is selected.
    pub system_prompt: Option<String>,
    /// Subjects producer copied onto the equal-input record.
    pub producer: Option<SubjectsProducer>,
}

/// Builds one payload and judges `backends` with it.
///
/// LLM backends receive one resolved system prompt. Jev receives `llm: None`.
/// Backends after the first also receive prior answers as session context
/// appended to that prompt. The canonical bytes stay shared, so the context
/// is not part of `prompt_sha256`.
///
/// When [`RunOptions::registry`] is `None`, keys are resolved with
/// [`sc_agent_secrets::resolve_key`] against [`RunOptions::repo_root`] and the
/// HTTP backends are constructed before any send. Dry-run skips that step.
///
/// # Errors
///
/// Returns [`JevError`] for config, input, prompt, key, session, or judge
/// failures. A validation failure before the first send does not log.
pub fn run_check(
    spec: &CheckSpec,
    backends: &[BackendId],
    opts: RunOptions,
) -> Result<CheckResult, JevError> {
    require_backends(backends)?;
    let built = assemble_payload(spec, &opts.repo_root, &opts.rev)?;
    let needs_llm = backends.iter().copied().any(is_llm);
    let system_prompt = prompt::load_system_prompt(
        spec,
        &opts.repo_root,
        &opts.rev,
        opts.system_prompt_override.as_deref(),
        needs_llm,
    )?;
    run_request(
        &RunRequest {
            bundle: built.bundle,
            system_prompt,
            producer: built.producer,
        },
        backends,
        opts,
    )
}

/// Judges `request` on `backends` with one shared payload.
///
/// # Errors
///
/// Returns [`JevError::ConfigInvalid`] when no backend is requested or an LLM
/// backend has no system prompt on `request`. A missing prompt file is
/// [`JevError::PromptFileMissing`] from [`run_check`], which carries the real
/// path. Validation or judge failures return the corresponding [`JevError`].
/// Dry-run returns the digest without resolving keys or sending.
pub fn run_request(
    request: &RunRequest,
    backends: &[BackendId],
    opts: RunOptions,
) -> Result<CheckResult, JevError> {
    require_backends(backends)?;
    let needs_llm = backends.iter().copied().any(is_llm);
    if needs_llm && request.system_prompt.is_none() {
        return Err(JevError::ConfigInvalid {
            detail: "an LLM backend requires a system prompt on the run request; pass system_prompt or load it with run_check".into(),
        });
    }
    if opts.dry_run {
        return Ok(CheckResult {
            session_id: None,
            bundle: request.bundle.clone(),
            system_prompt: request.system_prompt.clone(),
            outcomes: Vec::new(),
            equal_input: None,
        });
    }

    let registry = match &opts.registry {
        Some(registry) => registry.clone(),
        None => registry_for_repo(&opts.repo_root, backends)?,
    };
    opts.sessions.install_registry(registry.clone());
    for id in backends {
        let backend = registry.get(*id)?;
        let judge_request =
            judge_request(*id, &request.bundle, request.system_prompt.as_deref(), &[])?;
        backend.validate_for_send(&judge_request)?;
    }

    let primary = backends
        .iter()
        .copied()
        .find(|&id| is_llm(id))
        .unwrap_or(backends[0]);
    let session_id = opts
        .sessions
        .open(primary, &request.bundle, request.system_prompt.clone())?;
    let mut outcomes = Vec::with_capacity(backends.len());
    let mut payloads = Vec::with_capacity(backends.len());
    for id in backends {
        let prior = opts.sessions.load(&session_id)?.outcomes;
        let judge_request = judge_request(
            *id,
            &request.bundle,
            request.system_prompt.as_deref(),
            &prior,
        )?;
        payloads.push(judge_request.payload.bytes().to_vec());
        let backend = registry.get(*id)?;
        backend.validate_for_send(&judge_request)?;
        let judged = backend.judge(&judge_request);
        emit_log(
            &opts.session_log,
            LogEvent {
                session_id,
                backend: *id,
                prompt_sha256: request.bundle.sha256().clone(),
                operation: "judge".to_owned(),
            },
        );
        let outcome = judged?;
        opts.sessions.record_outcome(&session_id, &outcome)?;
        outcomes.push(outcome);
    }

    let equal_input = record(
        &request.bundle,
        &commands_for(backends),
        outcomes.len() != backends.len(),
        same_bytes(&payloads),
        request.producer.clone(),
    );
    Ok(CheckResult {
        session_id: Some(session_id),
        bundle: request.bundle.clone(),
        system_prompt: request.system_prompt.clone(),
        outcomes,
        equal_input: Some(equal_input),
    })
}

/// Caller questions document prepared for [`run_request`].
///
/// [`prepare_caller_request`] validates the document and builds the payload.
/// `sc-ai` passes the result to [`run_request`] and does not assemble bytes
/// itself (ADR-SAG-0005, ADR-SAG-0010).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedCallerRequest {
    /// Canonical payload shared by every backend.
    pub bundle: PayloadBundle,
    /// Backends to judge. CLI flags win over ids in the document.
    pub backends: Vec<BackendId>,
    /// LLM system prompt from the CLI file or the packaged default.
    ///
    /// `None` when no LLM backend is selected. The text is not part of
    /// [`PayloadBundle::sha256`].
    pub system_prompt: Option<String>,
}

/// Validates request JSON, builds one payload, and resolves the system prompt.
///
/// `flag_backends` are the `--backend` values. A non-empty slice replaces
/// backends named in the document. When both are empty the backend is
/// [`BackendId::Jev`]. Prompt precedence for this path is `prompt_file`, then
/// the packaged default. [`run_check`] applies `check.toml` on the check path.
/// A `system_prompt` field in `bytes` is refused (ADR-SAG-0019).
///
/// # Errors
///
/// Returns [`JevError`] when the document, backend id, prompt file, or
/// packaged asset is unusable. A Jev-only selection does not read
/// `prompt_file`.
pub fn prepare_caller_request(
    bytes: &[u8],
    flag_backends: &[BackendId],
    repo: &RepoRoot,
    prompt_file: Option<&Path>,
) -> Result<PreparedCallerRequest, JevError> {
    let parsed = crate::payload::parse_caller_request(bytes)?;
    let backends = if !flag_backends.is_empty() {
        flag_backends.to_vec()
    } else if !parsed.backends.is_empty() {
        parsed.backends
    } else {
        vec![BackendId::Jev]
    };
    let needs_llm = backends.iter().copied().any(is_llm);
    let system_prompt = prompt::load_request_system_prompt(repo, prompt_file, needs_llm)?;
    Ok(PreparedCallerRequest {
        bundle: parsed.bundle,
        backends,
        system_prompt,
    })
}

fn require_backends(backends: &[BackendId]) -> Result<(), JevError> {
    if backends.is_empty() {
        Err(JevError::ConfigInvalid {
            detail: "at least one backend is required".into(),
        })
    } else {
        Ok(())
    }
}

fn judge_request(
    id: BackendId,
    bundle: &PayloadBundle,
    base_prompt: Option<&str>,
    prior: &[JudgeOutcome],
) -> Result<JudgeRequest, JevError> {
    let llm = if is_llm(id) {
        let mut text = base_prompt.unwrap_or("").to_owned();
        text.push_str(&render_session_context(prior)?);
        Some(LlmWireContext {
            system_prompt: text,
        })
    } else {
        None
    };
    Ok(JudgeRequest {
        payload: bundle.clone(),
        llm,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{PACKAGED_SYSTEM_PROMPT, load_check_at, load_check_str};
    use crate::extract::test_support::{self, copy_fixture};
    use crate::session::{SESSION_CONTEXT_HEADER, SessionStore};
    use sc_agent_types::observability::v2::{LogEvent, NoopSessionLog, SessionLog};
    use sc_agent_types::{
        BackendKeyId, JevError, JudgeBackend, JudgeOutcome, JudgeRequest, ResolvedApiKey,
    };
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    const QUESTIONS: &str = r#"{
      "schema": "plan-anchoring/questions/1",
      "version": 1,
      "questions": {
        "relevant": {
          "type": "choice",
          "instructions": "Does it apply?",
          "criteria": {"no": "No.", "yes": "Yes."}
        }
      }
    }"#;

    struct Capture {
        id: BackendId,
        requests: Arc<Mutex<Vec<JudgeRequest>>>,
        answers: serde_json::Value,
    }

    impl JudgeBackend for Capture {
        fn id(&self) -> BackendId {
            self.id
        }

        fn validate_for_send(&self, request: &JudgeRequest) -> Result<(), JevError> {
            if is_llm(self.id) && request.llm.is_none() {
                return Err(JevError::InputInvalid {
                    detail: "LLM system prompt is required".into(),
                });
            }
            Ok(())
        }

        fn judge(&self, request: &JudgeRequest) -> Result<JudgeOutcome, JevError> {
            self.validate_for_send(request)?;
            self.requests
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(request.clone());
            Ok(JudgeOutcome {
                answers: self.answers.clone(),
                backend: self.id,
                probabilities: None,
                provider_meta: None,
            })
        }
    }

    struct JsonlSink {
        path: PathBuf,
    }

    impl SessionLog for JsonlSink {
        fn try_log(
            &self,
            event: LogEvent,
        ) -> Result<(), sc_agent_types::observability::v2::EventError> {
            use std::io::Write;
            let line = serde_json::json!({
                "session_id": event.session_id.to_string(),
                "backend": event.backend.to_string(),
                "prompt_sha256": event.prompt_sha256.as_str(),
                "operation": event.operation,
            });
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
                .expect("jsonl");
            writeln!(file, "{line}").expect("write jsonl");
            Ok(())
        }
    }

    fn bare_repo() -> test_support::TempDir {
        let dir = test_support::temp_dir("run");
        std::fs::write(dir.path().join("questions.json"), QUESTIONS).expect("questions");
        std::fs::write(
            dir.path().join("check.toml"),
            "[questions]\npath = \"questions.json\"\n",
        )
        .expect("check");
        dir
    }

    fn sessions_for(log: SharedSessionLog) -> (FileSessionStore, test_support::TempDir) {
        let dir = test_support::temp_dir("run-sessions");
        let store = FileSessionStore::new(dir.path(), Arc::clone(&log)).expect("sessions");
        (store, dir)
    }

    fn registry_of(captures: Vec<Capture>) -> BackendRegistry {
        let mut registry = BackendRegistry::new();
        for capture in captures {
            registry.register(Arc::new(capture)).expect("register");
        }
        registry
    }

    fn capture(id: BackendId, requests: &Arc<Mutex<Vec<JudgeRequest>>>, choice: &str) -> Capture {
        Capture {
            id,
            requests: Arc::clone(requests),
            answers: serde_json::json!({"relevant": {"choice": choice}}),
        }
    }

    #[test]
    fn run_dry_run_returns_digest_without_sending() {
        let repo = copy_fixture();
        let root = RepoRoot::new(repo.path());
        let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("check");
        let (sessions, _session_dir) = sessions_for(Arc::new(NoopSessionLog));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let registry = registry_of(vec![capture(BackendId::Jev, &seen, "yes")]);
        let result = run_check(
            &spec,
            &[BackendId::Jev],
            RunOptions::new(root, sessions, Arc::new(NoopSessionLog))
                .dry_run(true)
                .registry(registry),
        )
        .expect("dry-run");
        assert!(result.session_id.is_none());
        assert!(result.outcomes.is_empty());
        assert!(result.equal_input.is_none());
        assert_eq!(result.bundle.sha256().as_str().len(), 64);
        assert!(
            seen.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_empty()
        );
    }

    #[test]
    fn run_multi_backend_shares_one_payload_and_session_context() {
        let repo = bare_repo();
        let root = RepoRoot::new(repo.path());
        let text = std::fs::read_to_string(repo.path().join("check.toml")).expect("read");
        let spec = load_check_str(&text).expect("spec");
        let (sessions, _session_dir) = sessions_for(Arc::new(NoopSessionLog));
        let jev_seen = Arc::new(Mutex::new(Vec::new()));
        let haiku_seen = Arc::new(Mutex::new(Vec::new()));
        let registry = registry_of(vec![
            capture(BackendId::Jev, &jev_seen, "yes"),
            capture(BackendId::AnthropicHaiku, &haiku_seen, "no"),
        ]);
        let result = run_check(
            &spec,
            &[BackendId::Jev, BackendId::AnthropicHaiku],
            RunOptions::new(root, sessions.clone(), Arc::new(NoopSessionLog)).registry(registry),
        )
        .expect("run");
        let jev = jev_seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let haiku = haiku_seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(jev.len(), 1);
        assert_eq!(haiku.len(), 1);
        assert_eq!(jev[0].payload.bytes(), haiku[0].payload.bytes());
        assert_eq!(jev[0].payload.sha256(), result.bundle.sha256());
        assert!(jev[0].llm.is_none());
        let prompt = haiku[0].llm.as_ref().expect("haiku prompt");
        assert!(prompt.system_prompt.starts_with(PACKAGED_SYSTEM_PROMPT));
        assert!(prompt.system_prompt.contains(SESSION_CONTEXT_HEADER));
        assert!(prompt.system_prompt.contains("\"choice\":\"yes\""));
        assert!(!String::from_utf8_lossy(result.bundle.bytes()).contains(SESSION_CONTEXT_HEADER));
        assert!(
            !String::from_utf8_lossy(result.bundle.bytes()).contains("PACKAGED-PROMPT-TOKEN-a3")
        );
        let equal = result.equal_input.expect("equal");
        assert!(equal.equal);
        assert!(!equal.leak);
        assert_eq!(&equal.prompt_sha256, result.bundle.sha256());
        assert_eq!(equal.commands, ["jev", "anthropic_haiku"]);
        assert!(equal.producer.is_none());
        let session_id = result.session_id.expect("session");
        let stored = sessions.load(&session_id).expect("stored");
        assert_eq!(stored.outcomes.len(), 2);
        drop(jev);
        drop(haiku);
        let asked = sessions
            .ask(&session_id, "why the first answer?")
            .expect("ask");
        assert_eq!(asked.backend, BackendId::AnthropicHaiku);
    }

    #[test]
    fn run_packaged_prompt_is_injected_and_override_does_not_change_digest() {
        let repo = bare_repo();
        let root = RepoRoot::new(repo.path());
        let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("check");
        std::fs::write(repo.path().join("override.md"), "OVERRIDE-PROMPT-TOKEN").expect("override");
        let (sessions, _session_dir) = sessions_for(Arc::new(NoopSessionLog));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let first = run_check(
            &spec,
            &[BackendId::AnthropicHaiku],
            RunOptions::new(root.clone(), sessions, Arc::new(NoopSessionLog)).registry(
                registry_of(vec![capture(BackendId::AnthropicHaiku, &seen, "yes")]),
            ),
        )
        .expect("packaged");
        let prompt = seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)[0]
            .llm
            .as_ref()
            .expect("prompt")
            .system_prompt
            .clone();
        assert_eq!(prompt, PACKAGED_SYSTEM_PROMPT);
        drop(seen);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let (sessions, _session_dir) = sessions_for(Arc::new(NoopSessionLog));
        let second = run_check(
            &spec,
            &[BackendId::AnthropicHaiku],
            RunOptions::new(root, sessions, Arc::new(NoopSessionLog))
                .system_prompt_override(repo.path().join("override.md"))
                .registry(registry_of(vec![capture(
                    BackendId::AnthropicHaiku,
                    &seen,
                    "yes",
                )])),
        )
        .expect("override");
        let seen = seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let override_prompt = &seen[0].llm.as_ref().expect("override prompt").system_prompt;
        assert_eq!(override_prompt, "OVERRIDE-PROMPT-TOKEN");
        assert_eq!(first.bundle.sha256(), second.bundle.sha256());
    }

    #[test]
    fn run_check_file_prompt_beats_packaged_asset() {
        let repo = copy_fixture();
        let root = RepoRoot::new(repo.path());
        let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("check");
        let (sessions, _session_dir) = sessions_for(Arc::new(NoopSessionLog));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let _result = run_check(
            &spec,
            &[BackendId::OpenAiLuna],
            RunOptions::new(root, sessions, Arc::new(NoopSessionLog)).registry(registry_of(vec![
                capture(BackendId::OpenAiLuna, &seen, "yes"),
            ])),
        )
        .expect("run");
        let seen = seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let prompt = &seen[0].llm.as_ref().expect("prompt").system_prompt;
        assert!(prompt.contains("PROMPT-TOKEN-a3-check"));
        assert!(!prompt.contains("PACKAGED-PROMPT-TOKEN-a3"));
    }

    #[test]
    fn run_missing_llm_prompt_does_not_send() {
        let repo = bare_repo();
        std::fs::write(
            repo.path().join("check.toml"),
            "[questions]\npath = \"questions.json\"\n\n[llm_judge]\nsystem_prompt_file = \"missing.md\"\n",
        )
        .expect("check");
        let root = RepoRoot::new(repo.path());
        let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("check");
        let (sessions, session_dir) = sessions_for(Arc::new(NoopSessionLog));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let error = run_check(
            &spec,
            &[BackendId::AnthropicHaiku],
            RunOptions::new(root.clone(), sessions, Arc::new(NoopSessionLog)).registry(
                registry_of(vec![capture(BackendId::AnthropicHaiku, &seen, "yes")]),
            ),
        )
        .expect_err("missing prompt");
        assert_eq!(error.code(), "PROMPT_FILE_MISSING");
        assert!(
            seen.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_empty()
        );
        assert_eq!(
            std::fs::read_dir(session_dir.path()).expect("dir").count(),
            0
        );
        let (sessions, _session_dir) = sessions_for(Arc::new(NoopSessionLog));
        let jev = run_check(
            &spec,
            &[BackendId::Jev],
            RunOptions::new(root, sessions, Arc::new(NoopSessionLog)).registry(registry_of(vec![
                capture(BackendId::Jev, &Arc::new(Mutex::new(Vec::new())), "yes"),
            ])),
        )
        .expect("jev ignores missing prompt");
        assert!(jev.system_prompt.is_none());
        assert!(jev.outcomes[0].answers["relevant"]["choice"] == "yes");
    }

    #[test]
    fn run_refusing_backend_does_not_log() {
        let repo = bare_repo();
        let root = RepoRoot::new(repo.path());
        let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("check");
        let log_dir = test_support::temp_dir("jsonl");
        let log_path = log_dir.path().join("session.jsonl");
        let log = Arc::new(JsonlSink {
            path: log_path.clone(),
        });
        let (sessions, _session_dir) = sessions_for(Arc::clone(&log) as SharedSessionLog);
        let mut registry = BackendRegistry::new();
        registry
            .register(Arc::new(
                sc_agent_types::test_util::MockJudgeBackend::refusing(
                    BackendId::Jev,
                    JevError::InputTooLarge {
                        limit_bytes: 4,
                        actual_bytes: 8,
                    },
                ),
            ))
            .expect("register");
        let error = run_check(
            &spec,
            &[BackendId::Jev],
            RunOptions::new(root, sessions, log).registry(registry),
        )
        .expect_err("refused");
        assert_eq!(error.code(), "INPUT_TOO_LARGE");
        assert!(!log_path.exists());
    }

    #[test]
    fn run_log_lines_carry_correlation_and_no_secret() {
        let repo = bare_repo();
        std::fs::write(
            repo.path().join("override.md"),
            "prompt sk-test-DO_NOT_LOG_9f3a",
        )
        .expect("prompt");
        let root = RepoRoot::new(repo.path());
        let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("check");
        let log_dir = test_support::temp_dir("jsonl-ok");
        let log_path = log_dir.path().join("session.jsonl");
        let log = Arc::new(JsonlSink {
            path: log_path.clone(),
        });
        let (sessions, _session_dir) = sessions_for(Arc::clone(&log) as SharedSessionLog);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let result = run_check(
            &spec,
            &[BackendId::Jev, BackendId::AnthropicHaiku],
            RunOptions::new(root, sessions, Arc::clone(&log) as SharedSessionLog)
                .system_prompt_override(repo.path().join("override.md"))
                .registry(registry_of(vec![
                    capture(BackendId::Jev, &seen, "yes"),
                    capture(BackendId::AnthropicHaiku, &seen, "no"),
                ])),
        )
        .expect("run");
        let text = std::fs::read_to_string(&log_path).expect("jsonl");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        let session_id = result.session_id.expect("session").to_string();
        for (line, backend) in lines.iter().zip(["jev", "anthropic_haiku"]) {
            let value: serde_json::Value = serde_json::from_str(line).expect("line");
            assert_eq!(value["session_id"], session_id);
            assert_eq!(value["backend"], backend);
            assert_eq!(value["prompt_sha256"], result.bundle.sha256().as_str());
            assert_eq!(value["operation"], "judge");
            assert!(!line.contains("sk-test-DO_NOT_LOG_9f3a"));
            assert!(!line.to_ascii_lowercase().contains("authorization"));
            assert!(!line.to_ascii_lowercase().contains("bearer"));
        }
    }

    #[test]
    fn run_registry_accepts_local_backend_for_same_run_check_signature() {
        let repo = bare_repo();
        let root = RepoRoot::new(repo.path());
        let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("check");
        let (sessions, _session_dir) = sessions_for(Arc::new(NoopSessionLog));
        let seen = Arc::new(Mutex::new(0_u32));
        let local_seen = Arc::clone(&seen);
        struct LocalJudge {
            seen: Arc<Mutex<u32>>,
        }
        impl JudgeBackend for LocalJudge {
            fn id(&self) -> BackendId {
                BackendId::Jev
            }
            fn validate_for_send(&self, _request: &JudgeRequest) -> Result<(), JevError> {
                Ok(())
            }
            fn judge(&self, _request: &JudgeRequest) -> Result<JudgeOutcome, JevError> {
                *self
                    .seen
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
                Ok(JudgeOutcome {
                    answers: serde_json::json!({"relevant": {"choice": "yes"}}),
                    backend: BackendId::Jev,
                    probabilities: None,
                    provider_meta: None,
                })
            }
        }
        let mut registry = BackendRegistry::new();
        registry
            .register(Arc::new(LocalJudge { seen: local_seen }))
            .expect("register");
        let result = run_check(
            &spec,
            &[BackendId::Jev],
            RunOptions::new(root, sessions, Arc::new(NoopSessionLog)).registry(registry),
        )
        .expect("run");
        assert_eq!(
            *seen
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            1
        );
        assert_eq!(result.outcomes.len(), 1);
    }

    #[test]
    fn run_resolver_receives_repo_root_before_construction() {
        let dir = test_support::temp_dir("run-root");
        let repo = RepoRoot::new(dir.path());
        let mut seen = Vec::new();
        let registry = crate::registry::registry_from_resolver(
            &repo,
            &[BackendId::OpenAiLuna],
            &mut |id, root| {
                assert_eq!(root.as_path(), repo.as_path());
                seen.push(id);
                Ok(ResolvedApiKey::new("test-key".to_owned()))
            },
        )
        .expect("built");
        assert_eq!(seen, [BackendKeyId::OpenAi]);
        assert!(registry.contains(BackendId::OpenAiLuna));
        assert!(!format!("{registry:?}").contains("test-key"));
    }

    #[test]
    fn run_request_missing_llm_prompt_is_config_invalid() {
        let dir = test_support::temp_dir("run-request-prompt");
        let (sessions, _session_dir) = sessions_for(Arc::new(NoopSessionLog));
        let error = run_request(
            &RunRequest {
                bundle: PayloadBundle::from_canonical_bytes(
                    br#"{"questions":[],"state":[]}"#.to_vec(),
                ),
                system_prompt: None,
                producer: None,
            },
            &[BackendId::OpenAiLuna],
            RunOptions::new(
                RepoRoot::new(dir.path()),
                sessions,
                Arc::new(NoopSessionLog),
            ),
        )
        .expect_err("missing prompt");
        assert_eq!(error.code(), "CONFIG_INVALID");
        assert!(
            !matches!(error, JevError::PromptFileMissing { .. }),
            "a missing run-request prompt is not a prompt-file path"
        );
        let hint = error.fix_hint();
        assert!(hint.contains("system prompt"), "{hint}");
        assert!(!hint.contains("at system prompt"), "{hint}");
    }

    #[test]
    fn run_subjects_producer_is_copied_onto_equal_input() {
        let repo = test_support::temp_dir("subjects-run");
        std::fs::write(repo.path().join("questions.json"), QUESTIONS).expect("questions");
        let subjects = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/jev/subjects-schema/v1/minimal.json"),
        )
        .expect("fixture");
        std::fs::write(repo.path().join("subjects.json"), &subjects).expect("subjects");
        std::fs::write(
            repo.path().join("check.toml"),
            "[questions]\npath = \"questions.json\"\n\n[inputs.branches]\nkind = \"subjects\"\npath = \"subjects.json\"\n",
        )
        .expect("check");
        let root = RepoRoot::new(repo.path());
        let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("check");
        let (sessions, _session_dir) = sessions_for(Arc::new(NoopSessionLog));
        let result = run_check(
            &spec,
            &[BackendId::Jev],
            RunOptions::new(root, sessions, Arc::new(NoopSessionLog)).registry(registry_of(vec![
                capture(BackendId::Jev, &Arc::new(Mutex::new(Vec::new())), "yes"),
            ])),
        )
        .expect("run");
        let equal = result.equal_input.expect("equal");
        let producer = equal.producer.expect("producer");
        assert_eq!(producer.name, "sc-treesitter");
        assert_eq!(producer.mode, "matches");
        assert_eq!(&equal.prompt_sha256, result.bundle.sha256());
        assert!(equal.equal);
        assert!(!equal.leak);
    }

    #[test]
    fn run_command_input_copies_producer() {
        let repo = test_support::temp_dir("command-run");
        std::fs::write(repo.path().join("questions.json"), QUESTIONS).expect("questions");
        let subjects = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/jev/subjects-schema/v1/minimal.json"),
        )
        .expect("fixture");
        std::fs::write(repo.path().join("subjects.json"), subjects).expect("subjects");
        let python = test_support::python();
        let check = format!(
            "[questions]\npath = \"questions.json\"\n\n[inputs.live]\nkind = \"command\"\ncommand = [\"{python}\", \"-c\", \"print(open('subjects.json').read())\"]\ntimeout_seconds = 30\n"
        );
        std::fs::write(repo.path().join("check.toml"), check).expect("check");
        let root = RepoRoot::new(repo.path());
        let spec = load_check_at(&root, &Rev::worktree(), "check.toml").expect("check");
        let (sessions, _session_dir) = sessions_for(Arc::new(NoopSessionLog));
        let result = run_check(
            &spec,
            &[BackendId::Jev],
            RunOptions::new(root, sessions, Arc::new(NoopSessionLog)).registry(registry_of(vec![
                capture(BackendId::Jev, &Arc::new(Mutex::new(Vec::new())), "yes"),
            ])),
        )
        .expect("run");
        let producer = result
            .equal_input
            .expect("equal")
            .producer
            .expect("producer");
        assert_eq!(producer.name, "sc-treesitter");
        assert_eq!(producer.language, "python");
    }

    #[test]
    fn build_request_assembles_canonical_payload_and_producer() {
        let repo = test_support::temp_dir("build-request");
        std::fs::write(repo.path().join("questions.json"), QUESTIONS).expect("questions");
        let subjects = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/jev/subjects-schema/v1/minimal.json"),
        )
        .expect("fixture");
        std::fs::write(repo.path().join("subjects.json"), subjects).expect("subjects");
        std::fs::write(
            repo.path().join("check.toml"),
            "[questions]\npath = \"questions.json\"\n\n[inputs.branches]\nkind = \"subjects\"\npath = \"subjects.json\"\n",
        )
        .expect("check");
        let root = RepoRoot::new(repo.path());
        let built = build_request(&root, &Rev::worktree(), "check.toml").expect("build");
        let again = build_request(&root, &Rev::worktree(), "check.toml").expect("rebuild");
        assert_eq!(built.bundle.sha256(), again.bundle.sha256());
        assert_eq!(
            built.bundle.sha256(),
            &sc_agent_types::Sha256Digest::of(built.bundle.bytes())
        );
        let producer = built.producer.expect("producer");
        assert_eq!(producer.name, "sc-treesitter");
        assert_eq!(producer.mode, "matches");
        let missing = build_request(&root, &Rev::worktree(), "missing.toml").expect_err("missing");
        assert_eq!(missing.code(), "INPUT_MISSING");
    }

    #[test]
    fn prepare_caller_request_uses_file_override_not_inline_prompt() {
        let repo = test_support::temp_dir("caller-prompt");
        let root = RepoRoot::new(repo.path());
        let override_path = repo.path().join("override.md");
        std::fs::write(&override_path, "OVERRIDE-PROMPT-TOKEN-a7\n").expect("override");
        let inline = br#"{"questions":[{"id":"q","prompt":"p","choices":["no","yes"]}],"system_prompt":"INLINE-PROMPT-TOKEN"}"#;
        let rejected = prepare_caller_request(inline, &[], &root, None).expect_err("inline");
        assert!(!rejected.fix_hint().contains("INLINE-PROMPT-TOKEN"));
        assert!(rejected.fix_hint().contains("--llm-system-prompt-file"));

        let bytes = br#"{"questions":[{"id":"q","prompt":"Judge this","choices":["no","yes"]}],"backends":["anthropic_haiku"]}"#;
        let packaged = prepare_caller_request(bytes, &[], &root, None).expect("packaged");
        assert_eq!(packaged.backends, vec![BackendId::AnthropicHaiku]);
        assert_eq!(
            packaged.system_prompt.as_deref(),
            Some(PACKAGED_SYSTEM_PROMPT)
        );
        let jev_only = prepare_caller_request(
            bytes,
            &[BackendId::Jev],
            &root,
            Some(override_path.as_path()),
        )
        .expect("flags win");
        assert_eq!(jev_only.backends, vec![BackendId::Jev]);
        assert!(jev_only.system_prompt.is_none());
        let llm = prepare_caller_request(
            bytes,
            &[BackendId::AnthropicHaiku],
            &root,
            Some(override_path.as_path()),
        )
        .expect("override");
        assert_eq!(
            llm.system_prompt.as_deref(),
            Some("OVERRIDE-PROMPT-TOKEN-a7\n")
        );
        assert_eq!(packaged.bundle.sha256(), llm.bundle.sha256());
        let text = std::str::from_utf8(llm.bundle.bytes()).expect("utf8");
        assert!(!text.contains("OVERRIDE-PROMPT-TOKEN-a7"));
        assert!(text.contains("Judge this"));
    }
}
