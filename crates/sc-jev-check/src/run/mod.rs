//! `build_request`, `run_check`, and `run_request` (REQ-SAG-0601, REQ-SAG-0603).
//!
//! [`build_request`] is the public payload assembly entry (ADR-SAG-0005).
//! [`run_check`] uses that same assembly, resolves one LLM system prompt, and
//! judges every requested backend with those bytes. Dry-run returns the digest
//! and does not resolve keys or send. Judge sends emit one [`LogEvent`]
//! through the injected [`SharedSessionLog`].

#![warn(missing_docs)]

pub(crate) mod prompt;

use std::path::PathBuf;
use std::sync::Arc;

use sc_agent_types::observability::v2::{LogEvent, NoopSessionLog, SharedSessionLog};
use sc_agent_types::schema::subjects::SubjectsProducer;
use sc_agent_types::{
    Attestation, BackendId, EqualInputRecord, JevError, JudgeOutcome, JudgeRequest, LlmWireContext,
    MultiBackendMode, Operation, PayloadBundle, RepoRoot, Rev, SessionId,
};

use crate::config::CheckSpec;
use crate::equal_input::{RunAttestation, producer_from_extractions, record};
use crate::payload::PayloadBuilder;
#[doc(inline)]
pub use crate::payload::{PreparedCallerRequest, prepare_caller_request};
use crate::registry::{BackendRegistry, registry_for_repo};
use crate::session::{FileSessionStore, emit_log, render_session_context};

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
    /// Session files created for a live run. `None` is valid only for a
    /// dry-run, which creates no session (see [`Self::dry_run_only`]).
    pub sessions: Option<FileSessionStore>,
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
            sessions: Some(sessions),
            registry: None,
        }
    }

    /// Options for a dry-run at `WORKTREE` that needs no session store or logger.
    ///
    /// Nothing is written to disk: no session directory, session file, or
    /// log is created. A run built from these options with
    /// [`Self::dry_run`]`(false)` fails with `CONFIG_INVALID` before any send.
    #[must_use]
    pub fn dry_run_only(repo_root: RepoRoot) -> Self {
        Self {
            repo_root,
            dry_run: true,
            system_prompt_override: None,
            session_log: std::sync::Arc::new(NoopSessionLog),
            rev: Rev::worktree(),
            sessions: None,
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
        producer: producer_from_extractions(&built.extractions)?,
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
    let needs_llm = backends
        .iter()
        .any(|id| id.descriptor().requires_llm_context());
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
    let needs_llm = backends
        .iter()
        .any(|id| id.descriptor().requires_llm_context());
    if needs_llm && request.system_prompt.is_none() {
        return Err(run_request_invalid(
            "An LLM backend requires a system prompt on the run request. Pass system_prompt, or load it with run_check.",
        ));
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

    let Some(sessions) = opts.sessions.as_ref() else {
        return Err(run_request_invalid(
            "A live run needs a session store. Build RunOptions with RunOptions::new, or keep dry_run set.",
        ));
    };
    let registry = match &opts.registry {
        Some(registry) => registry.clone(),
        None => registry_for_repo(&opts.repo_root, backends)?,
    };
    sessions.install_registry(registry.clone());
    let payload = Arc::new(request.bundle.clone());
    for id in backends {
        let backend = registry.get(*id)?;
        let judge_request = judge_request(*id, &payload, request.system_prompt.as_deref(), &[])?;
        backend.validate_for_send(&judge_request)?;
    }

    let primary = backends
        .iter()
        .copied()
        .find(|id| id.descriptor().supports_session_ask())
        .unwrap_or(backends[0]);
    let session_id = sessions.open(primary, &request.bundle, request.system_prompt.clone())?;
    let mut outcomes = Vec::with_capacity(backends.len());
    for id in backends {
        let prior = sessions.load(&session_id)?.outcomes;
        let judge_request = judge_request(*id, &payload, request.system_prompt.as_deref(), &prior)?;
        let backend = registry.get(*id)?;
        // Pre-flight above validated the context-free request
        // (ADR-SAG-0004). Re-validate only when session context was appended.
        if id.descriptor().requires_llm_context() && !prior.is_empty() {
            backend.validate_for_send(&judge_request)?;
        }
        let judged = backend.judge(&judge_request);
        emit_log(
            &opts.session_log,
            LogEvent::new(
                session_id,
                *id,
                request.bundle.sha256().clone(),
                Operation::Judge,
            ),
        );
        let outcome = judged?;
        sessions.record_outcome(&session_id, &outcome)?;
        outcomes.push(outcome);
    }

    // Every backend got the same `Arc` bundle and later LLM backends got
    // earlier answers as session context (chain context). No backend reports
    // an input digest until r-2b, so each one is `Unattested`.
    let equal_input = record(
        &request.bundle,
        RunAttestation::uniform(
            MultiBackendMode::ChainContext,
            backends,
            &Attestation::Unattested,
            outcomes.len() != backends.len(),
        ),
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

fn require_backends(backends: &[BackendId]) -> Result<(), JevError> {
    if backends.is_empty() {
        Err(JevError::registry_invalid(
            "No backend was requested",
            "Request at least one backend: jev, anthropic_haiku, or openai_luna.",
        ))
    } else {
        Ok(())
    }
}

/// `CONFIG_INVALID` for a run request whose fix hint is `recovery` verbatim.
///
/// The failure is in how the caller built the run, not in `check.toml`, so
/// the `check.toml` hint would point at the wrong file.
fn run_request_invalid(recovery: &str) -> JevError {
    JevError::operational_config(recovery)
}

fn judge_request(
    id: BackendId,
    bundle: &Arc<PayloadBundle>,
    base_prompt: Option<&str>,
    prior: &[JudgeOutcome],
) -> Result<JudgeRequest, JevError> {
    let llm = if id.descriptor().requires_llm_context() {
        let mut text = base_prompt.unwrap_or("").to_owned();
        text.push_str(&render_session_context(prior)?);
        Some(LlmWireContext::new(text))
    } else {
        None
    };
    Ok(JudgeRequest::new(Arc::clone(bundle), llm))
}

#[cfg(test)]
mod tests;
