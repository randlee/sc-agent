//! The engine entry point: [`prepare`] then [`run`] (ADR-SAG-0024,
//! REQ-SAG-0620, REQ-SAG-0601).
//!
//! This is the only public way to run a check. [`prepare`] turns a
//! [`CheckSource`] and a [`RunConfig`] into a [`PreparedRun`]: it loads the
//! check, assembles the one canonical payload (ADR-SAG-0005), selects the
//! backends, and resolves the LLM system prompt when an LLM backend is
//! selected (ADR-SAG-0019). It reads only static `BackendDescriptor` facts
//! and constructs no backend. [`run`] consumes the [`PreparedRun`], so a run
//! always sees the config it was prepared with, and returns one
//! [`RunResponse`] whose serialization every consumer prints unchanged.
//! [`discover_repo_root`] is the one repo-root rule for every consumer
//! (ADR-SAG-0003 r-2 amendment).
//!
//! # Backend selection
//!
//! [`RunConfig::backends`] when set (CLI `--backend`, Python `backends=`),
//! else the source document's `backends` (`check.toml` or the caller
//! request), else `[jev]`.
//!
//! # Live runs
//!
//! `run` uses [`RunConfig::registry`] when set, else
//! [`BackendRegistry::from_env`]. It calls `validate_for_send` on every
//! selected backend before the first `judge` (ADR-SAG-0004): each backend
//! owns its request byte cap (REQ-SAG-0314). Backends are judged in
//! `BackendSet` order; a later LLM backend receives earlier answers as
//! session context appended to the system prompt, never to the canonical
//! bytes, and that request is re-validated before its own send. The run is
//! recorded as [`MultiBackendMode::ChainContext`] with every backend
//! `Unattested` (r-1 interim amendment of ADR-SAG-0008/0019; ADR-SAG-0025,
//! r-2b, changes both). In r-2 the run is fail-fast: the first backend
//! error is returned as `Err`. Each judge send emits one [`LogEvent`].
//!
//! The session is written once, at the end, only when a backend whose
//! descriptor has `supports_session_ask` returned `Ok`; that needs
//! [`RunConfig::sessions_dir`], checked before any send.
//!
//! # Dry runs
//!
//! [`RunConfig::dry_run`] sends nothing and constructs no backend. The
//! response carries the payload, `payload_bytes`, and per selected backend
//! its `request_limit_bytes` and `over_limit`; an over-limit payload is
//! reported, not refused. Every backend is `NotSent`, so `equal` is false.
//!
//! # Examples
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use sc_agent_types::observability::v2::NoopSessionLog;
//! use sc_jev_check::run::{CheckSource, RunConfig, discover_repo_root, prepare, run};
//!
//! let repo = discover_repo_root(std::path::Path::new("."))?;
//! let config = RunConfig::new(repo, Arc::new(NoopSessionLog))
//!     .backends("jev,anthropic_haiku".parse()?)
//!     .sessions_dir(".sc-ai/sessions");
//! let prepared = prepare(CheckSource::CheckFile("check.toml".into()), config)?;
//! println!("payload digest {}", prepared.prompt_sha256());
//! let response = run(prepared)?;
//! assert!(!response.is_partial());
//! # Ok::<(), sc_agent_types::JevError>(())
//! ```

#![warn(missing_docs)]

#[cfg(any(test, feature = "test-util"))]
mod builder;
pub(crate) mod prompt;
mod response;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sc_agent_types::observability::v2::{LogEvent, SharedSessionLog};
use sc_agent_types::schema::subjects::SubjectsProducer;
use sc_agent_types::{
    Attestation, BackendAttestation, BackendId, BackendSet, CancelToken, EqualInputRecord,
    JevError, JudgeBackend, JudgeOutcome, JudgeRequest, LlmWireContext, MultiBackendMode,
    Operation, PayloadBundle, RepoRoot, Rev, SessionId, Sha256Digest,
};

use crate::config::{CheckSpec, load_check_at};
use crate::equal_input::{RunAttestation, producer_from_extractions, record};
use crate::payload::build_payload;
use crate::payload::caller::parse_caller_request;
use crate::registry::BackendRegistry;
use crate::session::{FileSessionStore, StoredSession, emit_log, render_session_context};

#[cfg(any(test, feature = "test-util"))]
#[doc(inline)]
pub use builder::RunResponseBuilder;
use response::ResponseParts;
#[doc(inline)]
pub use response::{
    BackendFailure, BackendLimit, BackendResult, BackendStatus, ENVELOPE_VERSION, Envelope, OP_ASK,
    OP_DRY_RUN, OP_RUN, RunResponse,
};

/// Where a run's check comes from.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum CheckSource {
    /// A repo-relative `check.toml`, read at [`RunConfig::rev`].
    CheckFile(PathBuf),
    /// An already parsed check.
    Spec(CheckSpec),
    /// A caller-built questions document (`sc-ai jev --request-json`).
    CallerRequest(CallerRequest),
}

/// A validated caller-built questions document.
///
/// The document is `{"questions": [...], "state"?: [...], "backends"?: [...]}`;
/// it has no system-prompt field. Its payload is built here (ADR-SAG-0005).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallerRequest {
    bundle: PayloadBundle,
    backends: Option<BackendSet>,
}

impl CallerRequest {
    /// Parses and validates `bytes` and builds its canonical payload.
    ///
    /// # Errors
    ///
    /// `INPUT_INVALID` when the document is empty, not the questions shape,
    /// names an unknown or repeated backend, or includes `system_prompt`.
    pub fn from_json(bytes: &[u8]) -> Result<Self, JevError> {
        let parsed = parse_caller_request(bytes)?;
        let backends = if parsed.backends.is_empty() {
            None
        } else {
            Some(BackendSet::try_from(parsed.backends).map_err(|_| {
                JevError::input_invalid("request JSON backends must not repeat a backend")
            })?)
        };
        Ok(Self {
            bundle: parsed.bundle,
            backends,
        })
    }

    /// The canonical payload built from the document.
    #[must_use]
    pub fn bundle(&self) -> &PayloadBundle {
        &self.bundle
    }

    /// The document's `backends`, when it names any.
    #[must_use]
    pub fn backends(&self) -> Option<&BackendSet> {
        self.backends.as_ref()
    }
}

/// How to run a check. Built with [`RunConfig::new`] and the setters.
///
/// Fields are private: [`prepare`] takes the config by value and
/// [`PreparedRun`] owns it, so [`run`] cannot see a different one.
#[derive(Clone)]
pub struct RunConfig {
    repo_root: RepoRoot,
    session_log: SharedSessionLog,
    backends: Option<BackendSet>,
    rev: Rev,
    dry_run: bool,
    system_prompt_override: Option<PathBuf>,
    sessions_dir: Option<PathBuf>,
    registry: Option<Arc<BackendRegistry>>,
    cancel: CancelToken,
}

impl RunConfig {
    /// A live run at `WORKTREE` under `repo_root`, logging judge sends to
    /// `session_log`, with the backend selection left to the source.
    #[must_use]
    pub fn new(repo_root: RepoRoot, session_log: SharedSessionLog) -> Self {
        Self {
            repo_root,
            session_log,
            backends: None,
            rev: Rev::worktree(),
            dry_run: false,
            system_prompt_override: None,
            sessions_dir: None,
            registry: None,
            cancel: CancelToken::new(),
        }
    }

    /// Selects `backends`; wins over the source document's `backends`.
    #[must_use]
    pub fn backends(mut self, backends: BackendSet) -> Self {
        self.backends = Some(backends);
        self
    }

    /// Reads check inputs and the `check.toml` prompt file at `rev`.
    #[must_use]
    pub fn rev(mut self, rev: Rev) -> Self {
        self.rev = rev;
        self
    }

    /// When `on`, [`run`] sends nothing and reports limits instead.
    #[must_use]
    pub fn dry_run(mut self, on: bool) -> Self {
        self.dry_run = on;
        self
    }

    /// A working-tree prompt file that wins over `check.toml` and the
    /// packaged asset (ADR-SAG-0019). A relative path is under the repo root.
    #[must_use]
    pub fn system_prompt_override(mut self, path: impl Into<PathBuf>) -> Self {
        self.system_prompt_override = Some(path.into());
        self
    }

    /// Directory for session files. Required for a live run that selects an
    /// `ask`-capable backend.
    #[must_use]
    pub fn sessions_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.sessions_dir = Some(dir.into());
        self
    }

    /// Uses `registry` instead of [`BackendRegistry::from_env`]: the
    /// no-secrets path for embedders and tests (EXT-009, ADR-SAG-0022).
    #[must_use]
    pub fn registry(mut self, registry: Arc<BackendRegistry>) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Cancellation token carried on every judge request.
    #[must_use]
    pub fn cancel(mut self, token: CancelToken) -> Self {
        self.cancel = token;
        self
    }
}

impl std::fmt::Debug for RunConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RunConfig")
            .field("repo_root", &self.repo_root)
            .field("backends", &self.backends)
            .field("rev", &self.rev)
            .field("dry_run", &self.dry_run)
            .field("system_prompt_override", &self.system_prompt_override)
            .field("sessions_dir", &self.sessions_dir)
            .field("registry", &self.registry)
            .finish_non_exhaustive()
    }
}

/// A check ready to run: its config, payload, backends, and resolved prompt.
///
/// Produced by [`prepare`] and consumed by [`run`].
pub struct PreparedRun {
    config: RunConfig,
    request: JudgeRequest,
    backends: BackendSet,
    producer: Option<SubjectsProducer>,
}

impl PreparedRun {
    /// SHA-256 of the canonical payload.
    #[must_use]
    pub fn prompt_sha256(&self) -> &Sha256Digest {
        self.request.payload().sha256()
    }

    /// Size of the canonical payload in bytes.
    #[must_use]
    pub fn payload_bytes(&self) -> u64 {
        payload_len(self.request.payload())
    }

    /// Subjects producer from the first `subjects` or `command` input.
    #[must_use]
    pub fn producer(&self) -> Option<&SubjectsProducer> {
        self.producer.as_ref()
    }

    /// The base judge request: the shared payload and, when an LLM backend
    /// is selected, the resolved system prompt. A backend that takes no LLM
    /// context (Jev) is sent this payload with `llm: None`.
    #[must_use]
    pub fn request(&self) -> &JudgeRequest {
        &self.request
    }

    /// The selected backends, in judge order.
    #[must_use]
    pub fn backends(&self) -> &BackendSet {
        &self.backends
    }
}

impl std::fmt::Debug for PreparedRun {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedRun")
            .field("config", &self.config)
            .field("prompt_sha256", self.prompt_sha256())
            .field("backends", &self.backends)
            .finish_non_exhaustive()
    }
}

/// Loads the check, builds its payload, selects backends, and resolves the
/// LLM system prompt. Sends nothing and constructs no backend.
///
/// # Errors
///
/// Returns [`JevError`] when the check file or a document is missing or
/// invalid, an input fails, or an LLM backend is selected and its prompt
/// file is missing (`PROMPT_FILE_MISSING`) or empty.
pub fn prepare(source: CheckSource, config: RunConfig) -> Result<PreparedRun, JevError> {
    let (spec, bundle, producer, source_backends) = match source {
        CheckSource::CheckFile(path) => {
            let spec = load_check_at(&config.repo_root, &config.rev, &check_path_text(&path)?)?;
            assemble(spec, &config)?
        }
        CheckSource::Spec(spec) => assemble(spec, &config)?,
        CheckSource::CallerRequest(request) => (None, request.bundle, None, request.backends),
    };
    let backends = match (&config.backends, source_backends) {
        (Some(backends), _) => backends.clone(),
        (None, Some(backends)) => backends,
        (None, None) => default_backends(),
    };
    let needs_llm = backends
        .iter()
        .any(|id| id.descriptor().requires_llm_context());
    let override_path = config.system_prompt_override.as_deref();
    let system_prompt = match &spec {
        Some(spec) => prompt::load_system_prompt(
            spec,
            &config.repo_root,
            &config.rev,
            override_path,
            needs_llm,
        )?,
        None => prompt::load_request_system_prompt(&config.repo_root, override_path, needs_llm)?,
    };
    let request = JudgeRequest::new(Arc::new(bundle), system_prompt.map(LlmWireContext::new))
        .with_cancel(config.cancel.clone());
    Ok(PreparedRun {
        config,
        request,
        backends,
        producer,
    })
}

type Assembled = (
    Option<CheckSpec>,
    PayloadBundle,
    Option<SubjectsProducer>,
    Option<BackendSet>,
);

fn assemble(spec: CheckSpec, config: &RunConfig) -> Result<Assembled, JevError> {
    let built = build_payload(&spec, &config.repo_root, &config.rev)?;
    let producer = producer_from_extractions(&built.extractions)?;
    let backends = spec.backends.clone();
    Ok((Some(spec), built.bundle, producer, backends))
}

fn default_backends() -> BackendSet {
    BackendSet::try_from(vec![BackendId::Jev]).expect("[jev] is a valid backend set")
}

fn check_path_text(path: &Path) -> Result<String, JevError> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        JevError::input_invalid(format!("check path {} is not UTF-8", path.display()))
    })
}

fn payload_len(bundle: &PayloadBundle) -> u64 {
    u64::try_from(bundle.bytes().len()).unwrap_or(u64::MAX)
}

/// Runs `prepared` and returns its one [`RunResponse`].
///
/// See the module documentation for live and dry-run behaviour.
///
/// # Errors
///
/// A dry run fails only if the payload cannot be re-read as JSON (a bug).
/// A live run returns `CONFIG_INVALID` when an `ask`-capable backend is
/// selected and no sessions directory is set, or the session directory
/// cannot be created; the [`BackendRegistry::from_env`] errors (no
/// registry injected); `CONFIG_INVALID` when a selected backend is not in
/// the registry; the first `validate_for_send` refusal, before any send;
/// the first `judge` error; and a session write error. Every error before
/// the first send leaves no log line and no session file.
pub fn run(prepared: PreparedRun) -> Result<RunResponse, JevError> {
    if prepared.config.dry_run {
        dry_run_response(&prepared)
    } else {
        live_run(prepared)
    }
}

fn dry_run_response(prepared: &PreparedRun) -> Result<RunResponse, JevError> {
    let bundle = prepared.request.payload();
    let payload_bytes = payload_len(bundle);
    let payload = serde_json::from_slice(bundle.bytes())
        .map_err(|error| JevError::encode_failed("canonical payload", &error))?;
    let limits = prepared
        .backends
        .iter()
        .map(|id| BackendLimit::for_payload(id, payload_bytes))
        .collect();
    let equal_input = EqualInputRecord::new(
        bundle.sha256().clone(),
        MultiBackendMode::ChainContext,
        prepared
            .backends
            .iter()
            .map(|id| BackendAttestation::new(id, Attestation::NotSent))
            .collect(),
        false,
        prepared.producer.clone(),
    )?;
    Ok(RunResponse::from_parts(ResponseParts {
        session_id: None,
        payload_bytes,
        equal_input,
        results: Vec::new(),
        payload: Some(payload),
        limits,
    }))
}

fn live_run(prepared: PreparedRun) -> Result<RunResponse, JevError> {
    let PreparedRun {
        config,
        request,
        backends,
        producer,
    } = prepared;
    let store = open_session_store(&config, &backends)?;
    let registry = match &config.registry {
        Some(registry) => Arc::clone(registry),
        None => Arc::new(BackendRegistry::from_env(&config.repo_root, &backends)?),
    };
    let judges = backends
        .iter()
        .map(|id| registry.get(id).map(|judge| (id, judge)))
        .collect::<Result<Vec<(BackendId, Arc<dyn JudgeBackend>)>, JevError>>()?;
    // ADR-SAG-0004: every backend validates its base request (its own byte
    // cap, REQ-SAG-0314) before the first send.
    for (id, judge) in &judges {
        judge.validate_for_send(&request_for(*id, &request, &[])?)?;
    }

    let session_id = SessionId::new();
    let mut outcomes: Vec<JudgeOutcome> = Vec::with_capacity(judges.len());
    for (id, judge) in &judges {
        let judge_request = request_for(*id, &request, &outcomes)?;
        // Only an LLM request that gained session context differs from the
        // validated base request; re-validate it before its own send.
        if id.descriptor().requires_llm_context() && !outcomes.is_empty() {
            judge.validate_for_send(&judge_request)?;
        }
        let judged = judge.judge(&judge_request);
        emit_log(
            &config.session_log,
            LogEvent::new(
                session_id,
                *id,
                request.payload().sha256().clone(),
                Operation::Judge,
            ),
        );
        outcomes.push(judged?);
    }

    let saved = match &store {
        Some(store) => save_session(store, session_id, &request, &outcomes)?,
        None => false,
    };
    // Every backend got the same `Arc` bundle and later LLM backends got
    // earlier answers as session context (chain context). No backend's
    // digest is trusted until r-2b, so each one is `Unattested`.
    let equal_input = record(
        request.payload(),
        RunAttestation::uniform(
            MultiBackendMode::ChainContext,
            backends.as_slice(),
            &Attestation::Unattested,
            false,
        ),
        producer,
    )?;
    Ok(RunResponse::from_parts(ResponseParts {
        session_id: saved.then_some(session_id),
        payload_bytes: payload_len(request.payload()),
        equal_input,
        results: outcomes.into_iter().map(BackendResult::ok).collect(),
        payload: None,
        limits: Vec::new(),
    }))
}

/// The session store a live run may write, opened before any send.
fn open_session_store(
    config: &RunConfig,
    backends: &BackendSet,
) -> Result<Option<FileSessionStore>, JevError> {
    if !backends
        .iter()
        .any(|id| id.descriptor().supports_session_ask())
    {
        return Ok(None);
    }
    let Some(dir) = &config.sessions_dir else {
        return Err(JevError::operational_config(
            "A live run with an ask-capable backend saves a session. Set RunConfig::sessions_dir (sc-ai --sessions-dir), or use dry_run.",
        ));
    };
    FileSessionStore::new(dir).map(Some)
}

/// Writes the session once, when an `ask`-capable backend returned `Ok`.
fn save_session(
    store: &FileSessionStore,
    session_id: SessionId,
    request: &JudgeRequest,
    outcomes: &[JudgeOutcome],
) -> Result<bool, JevError> {
    let Some(backend) = outcomes
        .iter()
        .map(JudgeOutcome::backend)
        .find(|id| id.descriptor().supports_session_ask())
    else {
        return Ok(false);
    };
    store.save(&StoredSession {
        session_id,
        backend,
        created_at: store.now(),
        payload: request.payload().clone(),
        system_prompt: request.llm().map(|llm| llm.system_prompt.clone()),
        outcomes: outcomes.to_vec(),
        follow_ups: Vec::new(),
    })?;
    Ok(true)
}

/// The request `id` receives: the shared payload with no LLM context for a
/// backend that takes none (Jev), else the base system prompt with prior
/// answers appended as session context.
fn request_for(
    id: BackendId,
    base: &JudgeRequest,
    prior: &[JudgeOutcome],
) -> Result<JudgeRequest, JevError> {
    if !id.descriptor().requires_llm_context() {
        return Ok(JudgeRequest::new(Arc::clone(base.payload_arc()), None)
            .with_cancel(base.cancel().clone()));
    }
    if prior.is_empty() {
        return Ok(base.clone());
    }
    let mut text = base
        .llm()
        .map(|llm| llm.system_prompt.clone())
        .unwrap_or_default();
    text.push_str(&render_session_context(prior)?);
    Ok(JudgeRequest::new(
        Arc::clone(base.payload_arc()),
        Some(LlmWireContext::new(text)),
    )
    .with_cancel(base.cancel().clone()))
}

/// The repo root for `start`: its git toplevel, or `start` made absolute.
///
/// Uses `sc_agent_types::git::discover_toplevel`. Outside a git work tree,
/// or when git cannot be run, the result is `start` made absolute, so a
/// check in a plain directory still runs. Both consumers call this; the
/// `.env` used for keys is the one at this root (ADR-SAG-0003 r-2
/// amendment).
///
/// # Errors
///
/// `INPUT_INVALID` when `start` cannot be made absolute (the current
/// directory is unavailable).
pub fn discover_repo_root(start: &Path) -> Result<RepoRoot, JevError> {
    let absolute =
        std::path::absolute(start).map_err(|error| JevError::io_failed("repo root", &error))?;
    match sc_agent_types::git::discover_toplevel(&absolute) {
        Ok(Some(root)) => Ok(root),
        Ok(None) | Err(_) => Ok(RepoRoot::new(absolute)),
    }
}

#[cfg(test)]
mod tests;
