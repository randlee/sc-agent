//! The one place a backend is chosen and the runner is wired (ADR-SAG-0004 composition root).
//!
//! [`send`] and [`send_batch`] load the configuration, choose the backend (a configured local
//! runner, TypeSafe, or an LLM provider by ranking), build one `HttpClient` and one
//! `DecisionRunner`, and run the requests through the `Decision` trait. [`models`] lists the
//! TypeSafe or runner models. Every seam of [`Context`] is optional (REQ-SAG-0008).

use std::borrow::Cow;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use sc_agent_config::{AgentConfig, LocalRunner, RunnerName};
use sc_agent_http::{
    Attempts, HttpClient, HttpError, HttpRequest, HttpResponse, LogContext, Next, Pending,
    ResendPolicy, Start, Timeouts, Transport, UreqTransport,
};
use sc_agent_log::{GeneralLogger, Level, LogError, Repo, SessionIdentity, SessionLogger, Slug};
use sc_agent_secrets::{
    DeferredPlatformKeyring, EnvSource, KeySources, KeyringSource, ResolvedKey, SecretsError,
    resolve, resolve_var,
};
use sc_agent_types::{
    CancelToken, Clock, ErrorId, ModelChoice, ModelId, Provider, SessionId, ShortName, Wait,
};
use sc_decision_llm::LlmDecisionBackend;
use sc_decision_local::{DaemonKind, LocalBackend, LocalProbe, LocalTimeouts};
use sc_decision_runner::{DecisionRunner, RunnerConfig};
use sc_decision_schema::{
    Backend, Decision, DecisionError, DecisionRequest, DecisionResult, ModelMetadataList,
};
use sc_decision_systemone::{SystemOneBackend, TYPESAFE_BASE_URL};
use sc_llm_provider::{LlmClient, LlmProvider};
use serde_json::{Map, Value};

use crate::{DecidrError, RequestTimeout};

/// The general-log target of decidr's own records.
pub(crate) const LOG_TARGET: &str = "decidr";

/// Shared seams of every decidr operation; every field `None` is the real default
/// (REQ-SAG-0008).
#[derive(Clone)]
pub struct Context {
    /// The general-log service: `"decidr"` or `"sc-agent-py"`.
    pub service: &'static str,
    /// The configuration directory; `None` is `sc_agent_config::default_dir()` (`~/.sc-agent`).
    pub config_dir: Option<PathBuf>,
    /// The git work-tree top whose `.env` is read for keys (REQ-SAG-0401); `None` is
    /// `Repo::work_tree_root(cwd)`. Tests point it at a tempdir so no real `.env` is read.
    pub repo_root: Option<PathBuf>,
    /// `SC_AGENT__` overrides and key lookup; `None` is `std::env::vars_os()`.
    pub environment: Option<Vec<(OsString, OsString)>>,
    /// The keyring read last for keys; `None` is the platform keyring.
    pub keyring: Option<Arc<dyn KeyringSource + Send + Sync>>,
    /// Every send; `None` is a `UreqTransport` with the operation's timeouts.
    pub transport: Option<Arc<dyn Transport>>,
    /// The general log; `None` is `GeneralLogger::open_default(log_root, service)`.
    pub general: Option<GeneralLogger>,
    /// The clock of every decision run and of every wait (admissions, holdoffs, provider
    /// pauses, sends, the local load wait, probe holdoffs); `None` is the real clock
    /// ([`Clock::system`]). A test seam: with `sc_agent_types::testing::ManualClock`, a
    /// holdoff, pause or load poll falls due only when the test moves the clock.
    pub clock: Option<Clock>,
}

impl Context {
    /// A context for `service` with every seam `None`.
    #[must_use]
    pub fn new(service: &'static str) -> Self {
        Self {
            service,
            config_dir: None,
            repo_root: None,
            environment: None,
            keyring: None,
            transport: None,
            general: None,
            clock: None,
        }
    }

    /// The clock of this context: the injected one, else the real clock.
    pub(crate) fn clock(&self) -> Clock {
        self.clock.clone().unwrap_or_default()
    }

    /// The environment: the injected one borrowed, else the process environment read once.
    fn environment(&self) -> Cow<'_, [(OsString, OsString)]> {
        match &self.environment {
            Some(vars) => Cow::Borrowed(vars.as_slice()),
            None => Cow::Owned(std::env::vars_os().collect()),
        }
    }
}

impl fmt::Debug for Context {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Context")
            .field("service", &self.service)
            .field("config_dir", &self.config_dir)
            .field("repo_root", &self.repo_root)
            .field("environment", &self.environment.as_ref().map(Vec::len))
            .field("keyring", &self.keyring.is_some())
            .field("transport", &self.transport.is_some())
            .field("general", &self.general.is_some())
            .field("clock", &self.clock.is_some())
            .finish()
    }
}

/// What `--model` names, parsed once at the edge against the loaded configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelSelector {
    /// A line's latest model (`haiku`, `jev`, ...).
    Short(ShortName),
    /// One model id, used as given.
    Version(ModelId),
    /// A key of `config.local_runners`.
    Runner(RunnerName),
}

impl ModelSelector {
    /// A short name if `text` is one; else a configured local runner name; else a model id.
    ///
    /// # Errors
    ///
    /// [`DecidrError::Types`] (`types.invalid_model_id`) when `text` is none of them.
    pub fn parse(text: &str, config: &AgentConfig) -> Result<Self, DecidrError> {
        if let Ok(short) = text.parse::<ShortName>() {
            return Ok(Self::Short(short));
        }
        if let Some((name, _)) = config.local_runners.get_key_value(text) {
            return Ok(Self::Runner(name.clone()));
        }
        text.parse::<ModelId>()
            .map(Self::Version)
            .map_err(DecidrError::from)
    }
}

impl fmt::Display for ModelSelector {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Short(short) => fmt::Display::fmt(short, formatter),
            Self::Version(model) => fmt::Display::fmt(model, formatter),
            Self::Runner(name) => formatter.write_str(name.as_str()),
        }
    }
}

/// Options of [`send`] and [`send_batch`]; strings are parsed at the edge (RBP-F010).
#[derive(Clone)]
pub struct SendOptions {
    /// The model, line or local runner.
    pub model: ModelSelector,
    /// The provider override; `None` chooses by ranking among detected keys (REQ-SAG-0103).
    pub provider: Option<Provider>,
    /// The project slug of the session rows.
    pub project: Slug,
    /// The agent-role slug of the session rows.
    pub agent_role: Slug,
    /// The shared seams.
    pub context: Context,
    /// The session log; `None` is `SessionLogger::open_default(log_root, identity)`.
    pub session: Option<SessionLogger>,
}

impl fmt::Debug for SendOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SendOptions")
            .field("model", &self.model)
            .field("provider", &self.provider)
            .field("project", &self.project)
            .field("agent_role", &self.agent_role)
            .field("context", &self.context)
            .field("session", &self.session.is_some())
            .finish()
    }
}

/// Options of [`models`].
#[derive(Clone, Debug)]
pub struct ModelsOptions {
    /// A configured local runner; TypeSafe when `None`.
    pub runner: Option<RunnerName>,
    /// The shared seams.
    pub context: Context,
}

/// Loads the global configuration of `context` (REQ-SAG-0013 bounds included).
///
/// # Errors
///
/// [`DecidrError::Config`] with the `AgentConfigError` (key path in `config.invalid`).
pub fn load_config(context: &Context) -> Result<AgentConfig, DecidrError> {
    let dir = match &context.config_dir {
        Some(dir) => dir.clone(),
        None => sc_agent_config::default_dir()?,
    };
    // `load` takes the environment by value (A-4), so this is the one copy.
    Ok(sc_agent_config::load(
        dir,
        context.environment().into_owned(),
    )?)
}

/// The session identity of the working folder: `Repo::detect(cwd)`, `project`, `agent_role`.
///
/// # Errors
///
/// [`DecidrError::Log`] when the working folder or its repo cannot be determined.
pub fn session_identity(project: &Slug, agent_role: &Slug) -> Result<SessionIdentity, DecidrError> {
    let cwd = working_dir()?;
    let repo = Repo::detect(&cwd).map_err(DecidrError::from)?;
    SessionIdentity::new(repo, project.as_str(), agent_role.as_str()).map_err(DecidrError::from)
}

fn working_dir() -> Result<PathBuf, DecidrError> {
    std::env::current_dir().map_err(|_| {
        DecidrError::from(LogError::FolderUnknown {
            path: PathBuf::from("."),
        })
    })
}

/// The caller's environment as an [`EnvSource`]; the last setting of a name wins.
struct VarsEnv<'c>(Cow<'c, [(OsString, OsString)]>);

impl VarsEnv<'_> {
    fn raw(&self, name: &str) -> Option<&OsStr> {
        self.0
            .iter()
            .rev()
            .find(|(key, _)| key.as_os_str() == name)
            .map(|(_, value)| value.as_os_str())
    }

    /// A set variable whose value is not UTF-8 is a malformed key, never an empty one.
    fn check_utf8(&self, name: &str) -> Result<(), DecidrError> {
        match self.raw(name) {
            Some(value) if value.to_str().is_none() => {
                Err(DecidrError::from(SecretsError::KeyMalformed {
                    var: name.to_owned(),
                    reason: "the value is not UTF-8",
                }))
            }
            _ => Ok(()),
        }
    }
}

impl EnvSource for VarsEnv<'_> {
    fn var(&self, name: &str) -> Option<String> {
        // Non-UTF-8 values are refused by `check_utf8` before any lookup.
        self.raw(name).and_then(OsStr::to_str).map(str::to_owned)
    }
}

/// Key lookup sources of one operation: environment, repo-root `.env`, keyring (REQ-SAG-0401).
struct Keys<'c> {
    env: VarsEnv<'c>,
    repo_root: Option<PathBuf>,
    keyring: Arc<dyn KeyringSource + Send + Sync>,
}

impl<'c> Keys<'c> {
    fn new(context: &'c Context) -> Result<Self, DecidrError> {
        let cwd = working_dir()?;
        Ok(Self {
            env: VarsEnv(context.environment()),
            repo_root: match &context.repo_root {
                Some(root) => Some(root.clone()),
                None => Repo::work_tree_root(&cwd)?,
            },
            keyring: context
                .keyring
                .clone()
                .unwrap_or_else(|| Arc::new(DeferredPlatformKeyring)),
        })
    }

    fn sources(&self) -> KeySources<'_> {
        KeySources {
            env: &self.env,
            repo_root: self.repo_root.as_deref(),
            keyring: &*self.keyring,
        }
    }

    /// The key of `provider`; `Ok(None)` when no source has one.
    fn find(&self, provider: Provider) -> Result<Option<ResolvedKey>, DecidrError> {
        self.env.check_utf8(sc_agent_secrets::env_var(provider))?;
        match resolve(provider, &self.sources()) {
            Ok(key) => Ok(Some(key)),
            Err(SecretsError::KeyMissing { .. }) => Ok(None),
            Err(error) => Err(DecidrError::from(error)),
        }
    }

    /// The key of `provider`, which must exist.
    fn require(&self, provider: Provider) -> Result<ResolvedKey, DecidrError> {
        self.env.check_utf8(sc_agent_secrets::env_var(provider))?;
        Ok(resolve(provider, &self.sources())?)
    }

    /// The key held by variable `var` (a runner's `key_env`).
    fn require_var(&self, var: &sc_agent_types::VarName) -> Result<ResolvedKey, DecidrError> {
        self.env.check_utf8(var.as_str())?;
        Ok(resolve_var(var, &self.sources())?)
    }
}

/// The provider of a request and its key: the override, else the first of
/// `Provider::ranking(line)` with a detected key (REQ-SAG-0103).
fn choose_provider(
    choice: Option<Provider>,
    line: Option<ShortName>,
    model: &ModelSelector,
    keys: &Keys<'_>,
) -> Result<(Provider, ResolvedKey), DecidrError> {
    if let Some(provider) = choice {
        return Ok((provider, keys.require(provider)?));
    }
    let ranking = Provider::ranking(line);
    for provider in ranking {
        if let Some(key) = keys.find(*provider)? {
            return Ok((*provider, key));
        }
    }
    Err(DecidrError::NoProviderKey {
        model: model.clone(),
        tried: ranking.to_vec(),
    })
}

/// The general logger of `context`, or the default one for its service.
fn general_for(context: &Context, config: &AgentConfig) -> Result<GeneralLogger, DecidrError> {
    match &context.general {
        Some(general) => Ok(general.clone()),
        None => {
            let root = config.log_root()?;
            Ok(GeneralLogger::open_default(&root, context.service)?)
        }
    }
}

/// The general logger an operation over `context` writes to: the injected one, else
/// `<log_root>/logs/<service>.log.jsonl`.
///
/// Callers that own the process (the `decidr` binary, sc-agent-py) open it here, put it in
/// `context.general`, and close it after the operation, so every record is written (or with
/// the callable) before they return and a lost row is reported, not dropped.
///
/// # Errors
///
/// `Config` (the configuration or its log root) or `Log` (the logger cannot open).
pub fn general_logger(context: &Context) -> Result<GeneralLogger, DecidrError> {
    general_for(context, &load_config(context)?)
}

/// The session logger of `options`: the injected one, else
/// `<log_root>/logs/<repo>/<agent_role>/<project>/sessions.jsonl` (ADR-SAG-0002).
///
/// As [`general_logger`]: open it, put it in `options.session`, close it after the send.
///
/// # Errors
///
/// `Config` or `Log` (the repo, the identity or the logger).
pub fn session_logger(options: &SendOptions) -> Result<SessionLogger, DecidrError> {
    session_for(options, &load_config(&options.context)?)
}

fn session_for(options: &SendOptions, config: &AgentConfig) -> Result<SessionLogger, DecidrError> {
    match &options.session {
        Some(session) => Ok(session.clone()),
        None => {
            let identity = session_identity(&options.project, &options.agent_role)?;
            let root = config.log_root()?;
            Ok(SessionLogger::open_default(&root, identity)?)
        }
    }
}

/// The configured runner `name`.
fn local_runner<'c>(config: &'c AgentConfig, name: &str) -> Result<&'c LocalRunner, DecidrError> {
    config
        .local_runners
        .get(name)
        .ok_or_else(|| DecidrError::UnknownRunner {
            name: name.to_owned(),
            known: config.local_runners.keys().cloned().collect(),
        })
}

/// The runner's key, when its entry names `key_env`.
fn runner_key(
    runner: &LocalRunner,
    context: &Context,
) -> Result<Option<sc_agent_secrets::ApiKey>, DecidrError> {
    let Some(var) = &runner.key_env else {
        return Ok(None);
    };
    let keys = Keys::new(context)?;
    Ok(Some(keys.require_var(var)?.key))
}

/// A `LocalBackend` for `runner`, with its REQ-SAG-0015 timeouts.
fn local_backend(
    runner: &LocalRunner,
    context: &Context,
    timeout: Option<RequestTimeout>,
) -> Result<LocalBackend, DecidrError> {
    let key = runner_key(runner, context)?;
    let backend = LocalBackend::new(
        DaemonKind::from(runner.kind),
        &runner.host,
        runner.port,
        runner.model.clone(),
        key,
    )
    .map_err(DecidrError::from)?;
    Ok(backend.with_timeouts(LocalTimeouts {
        load: runner.load_timeout(),
        request: timeout.map_or_else(|| runner.request_timeout(), RequestTimeout::duration),
    }))
}

/// The backend a send uses.
enum Chosen {
    /// A configured local runner.
    Local {
        name: RunnerName,
        runner: LocalRunner,
        backend: Arc<LocalBackend>,
    },
    /// TypeSafe or an LLM provider.
    Remote {
        provider: Provider,
        model: ModelId,
        backend: Arc<dyn Backend>,
    },
}

impl Chosen {
    fn backend(&self) -> Arc<dyn Backend> {
        match self {
            Self::Local { backend, .. } => Arc::clone(backend) as Arc<dyn Backend>,
            Self::Remote { backend, .. } => Arc::clone(backend),
        }
    }

    /// `(provider, model, runner)` fields of the `decidr.send` record.
    fn label(&self) -> (String, String, Option<String>) {
        match self {
            Self::Local { name, runner, .. } => (
                "local".to_owned(),
                runner.model.as_str().to_owned(),
                Some(name.as_str().to_owned()),
            ),
            Self::Remote {
                provider, model, ..
            } => (
                provider.as_str().to_owned(),
                model.as_str().to_owned(),
                None,
            ),
        }
    }
}

/// Chooses the backend of `options` (the one place it is chosen).
///
/// A runner selector builds a `LocalBackend`; otherwise the model is resolved through the
/// config-built catalog (REQ-SAG-0101/0102) and the provider by [`choose_provider`]:
/// TypeSafe is a `SystemOneBackend`, any other provider an `LlmDecisionBackend`.
fn backend(
    options: &SendOptions,
    config: &AgentConfig,
    general: &GeneralLogger,
    timeout: Option<RequestTimeout>,
) -> Result<Chosen, DecidrError> {
    let (model, line) = match &options.model {
        ModelSelector::Runner(name) => {
            let runner = local_runner(config, name.as_str())?;
            let backend = local_backend(runner, &options.context, timeout)?;
            return Ok(Chosen::Local {
                name: name.clone(),
                runner: runner.clone(),
                backend: Arc::new(backend),
            });
        }
        ModelSelector::Short(short) => {
            let catalog = config.catalog().map_err(DecidrError::from)?;
            (catalog.resolve(&ModelChoice::Latest(*short)), Some(*short))
        }
        ModelSelector::Version(model) => {
            let catalog = config.catalog().map_err(DecidrError::from)?;
            (model.clone(), catalog.line_of(model))
        }
    };
    let keys = Keys::new(&options.context)?;
    let (provider, key) = choose_provider(options.provider, line, &options.model, &keys)?;
    let backend: Arc<dyn Backend> = match LlmProvider::from_provider(provider) {
        None => Arc::new(
            SystemOneBackend::new(TYPESAFE_BASE_URL, Some(key.key)).map_err(DecidrError::from)?,
        ),
        Some(llm) => {
            let client = LlmClient::new(llm, key.key)
                .map_err(DecidrError::from)?
                .with_log(general.clone());
            Arc::new(LlmDecisionBackend::new(client, model.clone()).with_log(general.clone()))
        }
    };
    Ok(Chosen::Remote {
        provider,
        model,
        backend,
    })
}

/// Which transport a run sends decisions over.
#[derive(Clone)]
enum TransportChoice {
    /// The context's injected transport.
    Injected(Arc<dyn Transport>),
    /// A `UreqTransport` with these timeouts.
    Ureq(Timeouts),
}

impl TransportChoice {
    fn build(self) -> Arc<dyn Transport> {
        match self {
            Self::Injected(transport) => transport,
            Self::Ureq(timeouts) => {
                #[cfg(test)]
                if let Some(recording) = tests::record_construction(timeouts) {
                    return recording;
                }
                Arc::new(UreqTransport::new(timeouts))
            }
        }
    }
}

/// The decision transport: the context's, else one `UreqTransport` per local runner with the
/// runner's request timeout (REQ-SAG-0015), else `Timeouts::default()`.
/// A per-call override replaces only the request timeout.
fn decision_transport(
    context: &Context,
    runner: Option<&LocalRunner>,
    timeout: Option<RequestTimeout>,
) -> TransportChoice {
    if let Some(transport) = &context.transport {
        return TransportChoice::Injected(Arc::clone(transport));
    }
    let defaults = match runner {
        Some(runner) => Timeouts::default().with_request(runner.request_timeout()),
        None => Timeouts::default(),
    };
    TransportChoice::Ureq(timeout.map_or(defaults, |value| defaults.with_request(value.duration())))
}

/// The probe transport (models, doctor, load probes): the context's, else `Timeouts::probe()`.
fn probe_transport(context: &Context) -> Arc<dyn Transport> {
    match &context.transport {
        Some(transport) => Arc::clone(transport),
        None => Arc::new(UreqTransport::new(Timeouts::probe())),
    }
}

/// One `HttpClient` (so the REQ-SAG-0012 provider pause is shared by every worker) and one
/// `DecisionRunner` over `backend`.
fn runner(
    clock: Clock,
    config: &AgentConfig,
    transport: Arc<dyn Transport>,
    session: SessionLogger,
    general: GeneralLogger,
    backend: Arc<dyn Backend>,
) -> Result<DecisionRunner, DecidrError> {
    // `runner.invalid_config` (workers out of bounds) maps through `DecidrError::Runner`.
    let runner_config = RunnerConfig::from_config(config)?;
    let http = HttpClient::new(
        transport,
        ResendPolicy::new(config.max_sends),
        general.clone(),
    );
    Ok(DecisionRunner::with_clock(
        backend,
        http,
        session,
        general,
        runner_config,
        clock,
    ))
}

/// Everything one send or batch needs, built before anything is sent.
struct Prepared {
    runner: DecisionRunner,
    general: GeneralLogger,
    /// Whether decidr opened `general` (the context had none), so decidr must flush it.
    owns_general: bool,
    chosen: Chosen,
    probe: Arc<dyn Transport>,
    clock: Clock,
    service: &'static str,
}

/// Whether the local model is loaded, or the wait was cancelled.
enum Loaded {
    Ready,
    Cancelled,
}

impl Prepared {
    fn new(options: &SendOptions, timeout: Option<RequestTimeout>) -> Result<Self, DecidrError> {
        let context = &options.context;
        let config = load_config(context)?;
        let general = general_for(context, &config)?;
        let chosen = backend(options, &config, &general, timeout)?;
        let session = session_for(options, &config)?;
        let local = match &chosen {
            Chosen::Local { runner, .. } => Some(runner),
            Chosen::Remote { .. } => None,
        };
        let transport = decision_transport(context, local, timeout).build();
        let runner = runner(
            context.clock(),
            &config,
            transport,
            session,
            general.clone(),
            chosen.backend(),
        )?;
        Ok(Self {
            runner,
            general,
            owns_general: context.general.is_none(),
            chosen,
            probe: probe_transport(context),
            clock: context.clock(),
            service: context.service,
        })
    }

    /// For a local runner, waits until its model is loaded before the first send
    /// (REQ-SAG-0015), so load time never counts toward a request timeout or the 90 s budget.
    fn wait_loaded(&self, cancel: &CancelToken) -> Result<Loaded, DecidrError> {
        let Chosen::Local {
            runner, backend, ..
        } = &self.chosen
        else {
            return Ok(Loaded::Ready);
        };
        let probe = LocalProbe::new(backend, &*self.probe, self.clock.clone())
            .with_log(self.general.clone());
        match wait_until_loaded(&probe, cancel, runner.load_timeout(), LogContext::NONE) {
            Ok(()) => Ok(Loaded::Ready),
            Err(DecisionError::Cancelled { .. }) => Ok(Loaded::Cancelled),
            Err(error) => Err(DecidrError::from(error)),
        }
    }

    /// Writes the `decidr.send` record, then flushes a general logger decidr opened: the
    /// runner's detached workers may hold its last handle, so a drop would never flush, and
    /// the file is created by the first write (QA-001). A lost row is returned after
    /// `outcome`'s own error, never instead of a result already handed out.
    fn finish(&self, tally: &Tally, outcome: Result<(), DecidrError>) -> Result<(), DecidrError> {
        self.record(tally, outcome.as_ref().err());
        let flushed = flush_owned(&self.general, self.owns_general);
        outcome.and(flushed)
    }

    /// Writes the one `decidr.send` record of this operation (never a key).
    fn record(&self, tally: &Tally, error: Option<&DecidrError>) {
        let (provider, model, runner) = self.chosen.label();
        let mut fields = Map::new();
        fields.insert("service".into(), self.service.into());
        fields.insert("model".into(), model.into());
        fields.insert("provider".into(), provider.into());
        if let Some(runner) = runner {
            fields.insert("runner".into(), runner.into());
        }
        fields.insert("requests".into(), tally.requests.into());
        fields.insert("succeeded".into(), tally.succeeded.into());
        fields.insert("failed".into(), tally.failed.into());
        fields.insert("cancelled".into(), tally.cancelled.into());
        if let Some(error) = error {
            fields.insert("error".into(), error.id().into());
        }
        // A failed write is counted by sc-agent-log and reported by its flush.
        let _ = self.general.write(
            Level::Info,
            LOG_TARGET,
            "decidr.send",
            LogContext::NONE,
            fields,
        );
    }
}

/// Flushes `general` when decidr opened it; the error (`log.rows_lost`, `log.flush`) as
/// `decidr.log`.
fn flush_owned(general: &GeneralLogger, owned: bool) -> Result<(), DecidrError> {
    if owned {
        general.flush()?;
    }
    Ok(())
}

/// `LocalProbe::wait_until_loaded` with the load timeout decidr passes. In tests the timeout
/// is also recorded on the calling thread (`LOAD_TIMEOUT_PASSED`), so a test can prove the
/// runner's configured `load_timeout_secs` is what reaches the wait, without waiting it out
/// (QA-006).
fn wait_until_loaded(
    probe: &LocalProbe<'_>,
    cancel: &CancelToken,
    load_timeout: std::time::Duration,
    ctx: LogContext,
) -> Result<(), DecisionError> {
    #[cfg(test)]
    LOAD_TIMEOUT_PASSED.with(|passed| passed.set(Some(load_timeout)));
    probe.wait_until_loaded(cancel, load_timeout, ctx)
}

#[cfg(test)]
thread_local! {
    /// The load timeout of the latest `wait_until_loaded` on this thread (test seam).
    static LOAD_TIMEOUT_PASSED: std::cell::Cell<Option<std::time::Duration>> =
        const { std::cell::Cell::new(None) };
}

/// Result counts of one operation.
#[derive(Debug, Default)]
struct Tally {
    requests: u64,
    succeeded: u64,
    failed: u64,
    cancelled: u64,
}

impl Tally {
    fn add(&mut self, result: &DecisionResult) {
        self.requests = self.requests.saturating_add(1);
        match &result.outcome {
            Ok(_) => self.succeeded = self.succeeded.saturating_add(1),
            Err(DecisionError::Cancelled { .. }) => {
                self.cancelled = self.cancelled.saturating_add(1);
            }
            Err(_) => self.failed = self.failed.saturating_add(1),
        }
    }
}

/// The result of a request never sent because the load wait was cancelled.
fn cancelled_before_send(request: &DecisionRequest, position: u64) -> DecisionResult {
    DecisionResult::new(
        SessionId::new(),
        position,
        request.digests().clone(),
        Err(DecisionError::Cancelled { sends: 0 }),
    )
}

/// Sends one request: always `Decision::decide` (CRIT-M3); `on_result` gets its result, then
/// the session log (and a general log decidr opened) is flushed, and one `decidr.send`
/// record is written.
///
/// A failed decision is data in the result. A flush failure after the result was handed out
/// is returned after it, never instead of it (QA-006), as in [`send_batch`].
///
/// # Errors
///
/// Before any send: `Config`, `Types`, `Secrets`, `Log`, `Local`, `Llm`, `Decision` (a local
/// runner that does not load), `Runner` (`runner.invalid_config`), `NoProviderKey`,
/// `UnknownRunner`. After it: `on_result`'s error, else [`DecidrError::Runner`] when the
/// session log cannot be flushed, else [`DecidrError::Log`] when the general log lost rows.
pub fn send(
    options: &SendOptions,
    request: DecisionRequest,
    cancel: &CancelToken,
    on_result: &mut dyn FnMut(DecisionResult) -> Result<(), DecidrError>,
) -> Result<(), DecidrError> {
    send_with_timeout(options, request, cancel, None, on_result)
}

/// As [`send`], with an optional checked per-call request timeout.
/// The override changes the default decision transport and local request deadline;
/// local load/probe timeouts and injected transports retain their own controls.
///
/// # Errors
/// Returns the same errors as [`send`].
pub fn send_with_timeout(
    options: &SendOptions,
    request: DecisionRequest,
    cancel: &CancelToken,
    timeout: Option<RequestTimeout>,
    on_result: &mut dyn FnMut(DecisionResult) -> Result<(), DecidrError>,
) -> Result<(), DecidrError> {
    let prepared = Prepared::new(options, timeout)?;
    let mut tally = Tally::default();
    let loaded = match prepared.wait_loaded(cancel) {
        Ok(loaded) => loaded,
        Err(error) => return prepared.finish(&tally, Err(error)),
    };
    let result = match loaded {
        Loaded::Ready => send_on(&prepared.runner, request, cancel),
        Loaded::Cancelled => cancelled_before_send(&request, 0),
    };
    tally.add(&result);
    let delivered = on_result(result);
    let flushed = prepared
        .runner
        .flush_session_log()
        .map_err(DecidrError::from);
    prepared.finish(&tally, delivered.and(flushed))
}

/// Sends a batch: always `Decision::decide_stream` with an owned iterator (CRIT-M3), even
/// for one request; `on_result` gets each result as it finishes (fastest first).
///
/// An `Err` from `on_result` cancels `cancel`; the rest is drained and dropped,
/// `flush_session_log` runs, and that `Err` is returned.
///
/// # Errors
///
/// As [`send`] before any send; then the first `on_result` error, else
/// [`DecidrError::Runner`] when the session log cannot be flushed.
pub fn send_batch(
    options: &SendOptions,
    requests: Vec<DecisionRequest>,
    cancel: &CancelToken,
    on_result: &mut dyn FnMut(DecisionResult) -> Result<(), DecidrError>,
) -> Result<(), DecidrError> {
    send_batch_with_timeout(options, requests, cancel, None, on_result)
}

/// As [`send_batch`], with an optional checked per-call request timeout.
/// The override changes the default decision transport and local request deadline;
/// local load/probe timeouts and injected transports retain their own controls.
///
/// # Errors
/// Returns the same errors as [`send_batch`].
pub fn send_batch_with_timeout(
    options: &SendOptions,
    requests: Vec<DecisionRequest>,
    cancel: &CancelToken,
    timeout: Option<RequestTimeout>,
    on_result: &mut dyn FnMut(DecisionResult) -> Result<(), DecidrError>,
) -> Result<(), DecidrError> {
    let prepared = Prepared::new(options, timeout)?;
    let mut tally = Tally::default();
    let loaded = match prepared.wait_loaded(cancel) {
        Ok(loaded) => loaded,
        Err(error) => return prepared.finish(&tally, Err(error)),
    };
    let mut counted = |result: DecisionResult| {
        tally.add(&result);
        on_result(result)
    };
    let outcome = match loaded {
        Loaded::Ready => send_batch_on(&prepared.runner, requests, cancel, &mut counted),
        Loaded::Cancelled => deliver(
            requests
                .iter()
                .zip(0_u64..)
                .map(|(request, position)| cancelled_before_send(request, position)),
            cancel,
            &mut counted,
        ),
    };
    let flushed = prepared
        .runner
        .flush_session_log()
        .map_err(DecidrError::from);
    prepared.finish(&tally, outcome.and(flushed))
}

/// [`send`]'s core over any `Decision`: one `decide` call.
#[must_use]
pub fn send_on(
    decision: &dyn Decision,
    request: DecisionRequest,
    cancel: &CancelToken,
) -> DecisionResult {
    decision.decide(request, cancel)
}

/// [`send_batch`]'s core over any `Decision`: one `decide_stream` call with
/// `Box::new(requests.into_iter())`.
///
/// # Errors
///
/// The first `on_result` error; the token is cancelled and the stream drained.
pub fn send_batch_on(
    decision: &dyn Decision,
    requests: Vec<DecisionRequest>,
    cancel: &CancelToken,
    on_result: &mut dyn FnMut(DecisionResult) -> Result<(), DecidrError>,
) -> Result<(), DecidrError> {
    let stream = decision.decide_stream(Box::new(requests.into_iter()), cancel);
    deliver(stream, cancel, on_result)
}

/// Hands each result to `on_result`; after its first error, cancels and drains the rest.
fn deliver(
    results: impl Iterator<Item = DecisionResult>,
    cancel: &CancelToken,
    on_result: &mut dyn FnMut(DecisionResult) -> Result<(), DecidrError>,
) -> Result<(), DecidrError> {
    let mut first_error = None;
    for result in results {
        if first_error.is_some() {
            continue;
        }
        if let Err(error) = on_result(result) {
            cancel.cancel();
            first_error = Some(error);
        }
    }
    first_error.map_or(Ok(()), Err)
}

/// Sends `request` once through `http`, waiting out holdoffs on `cancel` (A-5).
///
/// A busy or timed-out reply is resent under the client's policy; every wait is a
/// `CancelToken::wait_until`, never a sleep.
fn send_probe(
    http: &HttpClient,
    request: &HttpRequest,
    clock: &Clock,
    cancel: &CancelToken,
) -> Result<HttpResponse, DecidrError> {
    let cancelled = |sends: u32| DecidrError::from(DecisionError::Cancelled { sends });
    let mut pending: Option<Pending> = None;
    loop {
        if cancel.is_cancelled() {
            return Err(cancelled(
                pending.as_ref().map_or(0, |held| held.attempts().sends()),
            ));
        }
        let now = clock.now();
        let permit = match pending.take() {
            None => match http.may_start(request, Attempts::new(), now) {
                Ok(Start::Go(permit)) => permit,
                Ok(Start::Wait { at, .. }) => {
                    wait(clock, cancel, at, 0)?;
                    continue;
                }
                Err(reason) => {
                    return Err(DecidrError::from(HttpError::invalid_request(format!(
                        "the send was refused before it started ({})",
                        reason.as_str()
                    ))));
                }
            },
            Some(held) => match http.may_resend(request, held, now) {
                Ok(Start::Go(permit)) => permit,
                Ok(Start::Wait { at, state }) => {
                    let sends = state.attempts().sends();
                    pending = Some(state);
                    wait(clock, cancel, at, sends)?;
                    continue;
                }
                Err(error) => return Err(DecidrError::from(error)),
            },
        };
        match http
            .send_once(request, permit, LogContext::NONE, clock)
            .map_err(DecidrError::from)?
        {
            Next::Done { response, .. } => return Ok(response),
            Next::ResendAt { at, pending: held } => {
                let sends = held.attempts().sends();
                pending = Some(held);
                wait(clock, cancel, at, sends)?;
            }
        }
    }
}

/// Waits until `at` or a cancel.
fn wait(clock: &Clock, cancel: &CancelToken, at: Instant, sends: u32) -> Result<(), DecidrError> {
    match clock.wait_until(cancel, at) {
        Wait::Cancelled => Err(DecidrError::from(DecisionError::Cancelled { sends })),
        Wait::DeadlineReached => Ok(()),
    }
}

/// A one-off client on the probe transport.
pub(crate) fn probe_client(
    context: &Context,
    config: &AgentConfig,
    general: &GeneralLogger,
) -> HttpClient {
    HttpClient::new(
        probe_transport(context),
        ResendPolicy::new(config.max_sends),
        general.clone(),
    )
}

/// Sends a probe-class request (models, doctor) and returns the reply.
pub(crate) fn probe(
    http: &HttpClient,
    request: &HttpRequest,
    clock: &Clock,
    cancel: &CancelToken,
) -> Result<HttpResponse, DecidrError> {
    send_probe(http, request, clock, cancel)
}

/// The TypeSafe `SystemOneBackend` with its resolved key.
pub(crate) fn typesafe_backend(key: ResolvedKey) -> Result<SystemOneBackend, DecidrError> {
    SystemOneBackend::new(TYPESAFE_BASE_URL, Some(key.key)).map_err(DecidrError::from)
}

/// Key lookup for doctor: every provider in `Provider::PRIORITY` order with a key.
pub(crate) fn detected_keys(
    context: &Context,
) -> Result<Vec<(Provider, ResolvedKey)>, DecidrError> {
    let keys = Keys::new(context)?;
    let mut found = Vec::new();
    for provider in Provider::PRIORITY {
        if let Some(key) = keys.find(provider)? {
            found.push((provider, key));
        }
    }
    Ok(found)
}

/// The config and general logger of a probe-class operation, and whether decidr opened the
/// logger (then it must flush it, [`flush_owned`]).
pub(crate) fn probe_setup(
    context: &Context,
) -> Result<(AgentConfig, GeneralLogger, bool), DecidrError> {
    let config = load_config(context)?;
    let general = general_for(context, &config)?;
    Ok((config, general, context.general.is_none()))
}

/// `result`, unless a general logger decidr opened lost rows (then that error).
pub(crate) fn flushed_result<T>(
    result: Result<T, DecidrError>,
    general: &GeneralLogger,
    owned: bool,
) -> Result<T, DecidrError> {
    let flushed = flush_owned(general, owned);
    result.and_then(|value| flushed.map(|()| value))
}

/// GET /v1/models of TypeSafe or a configured runner, sent once with `Timeouts::probe()`
/// (A-5); writes one `decidr.models` record.
///
/// # Errors
///
/// `Config`, `Log`, `UnknownRunner`, `Secrets` (TypeSafe key), `Local`, `Decision` (the
/// request or the reply), `Http` (the send).
pub fn models(
    options: &ModelsOptions,
    cancel: &CancelToken,
) -> Result<ModelMetadataList, DecidrError> {
    let (config, general, owned) = probe_setup(&options.context)?;
    let result = list_models(options, &config, &general, cancel);
    let mut fields = Map::new();
    fields.insert("service".into(), options.context.service.into());
    fields.insert(
        "runner".into(),
        options
            .runner
            .as_ref()
            .map_or("typesafe", RunnerName::as_str)
            .into(),
    );
    fields.insert(
        "models".into(),
        result
            .as_ref()
            .map_or(Value::Null, |list| list.models.len().into()),
    );
    fields.insert(
        "outcome".into(),
        result
            .as_ref()
            .map_or_else(|error| error.id(), |_| "ok")
            .into(),
    );
    let _ = general.write(
        Level::Info,
        LOG_TARGET,
        "decidr.models",
        LogContext::NONE,
        fields,
    );
    flushed_result(result, &general, owned)
}

fn list_models(
    options: &ModelsOptions,
    config: &AgentConfig,
    general: &GeneralLogger,
    cancel: &CancelToken,
) -> Result<ModelMetadataList, DecidrError> {
    let backend = match &options.runner {
        Some(name) => {
            let runner = local_runner(config, name.as_str())?;
            let key = runner_key(runner, &options.context)?;
            let base = LocalBackend::new(
                DaemonKind::from(runner.kind),
                &runner.host,
                runner.port,
                runner.model.clone(),
                None,
            )
            .map_err(DecidrError::from)?
            .base()
            .to_owned();
            SystemOneBackend::local(&base, key).map_err(DecidrError::from)?
        }
        None => {
            let keys = Keys::new(&options.context)?;
            let key = resolve(Provider::TypeSafe, &keys.sources()).map_err(DecidrError::from)?;
            typesafe_backend(key)?
        }
    };
    let request = backend.models_request().map_err(DecidrError::from)?;
    let http = probe_client(&options.context, config, general);
    let response = probe(&http, &request, &options.context.clock(), cancel)?;
    backend.decode_models(response).map_err(DecidrError::from)
}

#[cfg(test)]
mod select_tests {
    use super::*;

    struct NoKeyring;

    impl KeyringSource for NoKeyring {
        fn password(&self, _: &str, _: &str) -> Result<Option<String>, SecretsError> {
            Ok(None)
        }
    }

    fn keys(vars: &[(&str, &str)]) -> Keys<'static> {
        Keys {
            env: VarsEnv(Cow::Owned(
                vars.iter()
                    .map(|(name, value)| (OsString::from(name), OsString::from(value)))
                    .collect(),
            )),
            repo_root: None,
            keyring: Arc::new(NoKeyring),
        }
    }

    /// AC: REQ-SAG-0103 (provider choice cases).
    #[test]
    fn the_first_ranked_provider_with_a_key_is_chosen() {
        let both = keys(&[
            (sc_agent_secrets::ANTHROPIC_API_KEY, "sk-a"),
            (sc_agent_secrets::OPENROUTER_API_KEY, "sk-o"),
        ]);
        let haiku = Some(ShortName::Haiku);
        let (provider, _) =
            choose_provider(None, haiku, &ModelSelector::Short(ShortName::Haiku), &both)
                .expect("anthropic");
        assert_eq!(provider, Provider::Anthropic);

        let reseller = keys(&[(sc_agent_secrets::OPENROUTER_API_KEY, "sk-o")]);
        let (provider, _) = choose_provider(
            None,
            haiku,
            &ModelSelector::Short(ShortName::Haiku),
            &reseller,
        )
        .expect("openrouter");
        assert_eq!(provider, Provider::OpenRouter);

        let (provider, _) = choose_provider(
            Some(Provider::OpenRouter),
            haiku,
            &ModelSelector::Short(ShortName::Haiku),
            &both,
        )
        .expect("override");
        assert_eq!(provider, Provider::OpenRouter);

        let error = choose_provider(
            None,
            Some(ShortName::Jev),
            &ModelSelector::Short(ShortName::Jev),
            &both,
        )
        .expect_err("no key");
        assert_eq!(error.id(), "decidr.no_provider_key");
        assert!(error.recovery().is_some());
        assert!(matches!(
            error,
            DecidrError::NoProviderKey { ref tried, .. } if tried == &[Provider::TypeSafe]
        ));
    }

    #[test]
    fn an_override_without_its_key_is_a_secrets_error() {
        let none = keys(&[]);
        let error = choose_provider(
            Some(Provider::OpenAi),
            Some(ShortName::Luna),
            &ModelSelector::Short(ShortName::Luna),
            &none,
        )
        .expect_err("no key");
        assert_eq!(error.id(), "decidr.secrets");
        assert_eq!(
            error.cause_detail().map(|cause| cause.id),
            Some("secrets.key_missing".to_owned())
        );
    }

    /// RBP-F008: a key variable that is set to a non-UTF-8 value is a malformed key, not an
    /// empty one.
    #[cfg(unix)]
    #[test]
    fn a_non_utf8_key_value_is_an_error() {
        use std::os::unix::ffi::OsStringExt;
        let keys = Keys {
            env: VarsEnv(Cow::Owned(vec![(
                OsString::from(sc_agent_secrets::TYPESAFE_API_KEY),
                OsString::from_vec(vec![0x74, 0xff, 0x73]),
            )])),
            repo_root: None,
            keyring: Arc::new(NoKeyring),
        };
        let error = keys.find(Provider::TypeSafe).expect_err("not UTF-8");
        assert_eq!(error.id(), "decidr.secrets");
        assert_eq!(
            error.cause_detail().map(|cause| cause.id),
            Some("secrets.key_malformed".to_owned())
        );
    }

    /// QA-007: `--model` precedence is short name, then configured runner, then model id. A
    /// runner cannot be named like a short name (`haiku`): the config refuses it, so `haiku`
    /// always means the line. A runner named like a model id wins over the model id.
    #[test]
    fn a_runner_follows_the_documented_precedence() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(
            dir.path().join("default.json"),
            r#"{"local_runners":{"haiku":{"kind":"laya","host":"127.0.0.1","port":9000,"model":"m"}}}"#,
        )
        .expect("write");
        let error = sc_agent_config::load(dir.path().to_path_buf(), Vec::new())
            .expect_err("a runner named like a short name");
        assert_eq!(error.id(), "config.runner_name");

        std::fs::write(
            dir.path().join("default.json"),
            r#"{"local_runners":{"claude-haiku-5-5":{"kind":"laya","host":"127.0.0.1","port":9000,"model":"m"}}}"#,
        )
        .expect("write");
        let config = sc_agent_config::load(dir.path().to_path_buf(), Vec::new()).expect("config");
        assert!(matches!(
            ModelSelector::parse("claude-haiku-5-5", &config),
            Ok(ModelSelector::Runner(name)) if name.as_str() == "claude-haiku-5-5"
        ));
        assert_eq!(
            ModelSelector::parse("haiku", &config).expect("short"),
            ModelSelector::Short(ShortName::Haiku)
        );
        assert!(matches!(
            ModelSelector::parse("claude-sonnet-5-5", &config),
            Ok(ModelSelector::Version(_))
        ));
    }

    #[test]
    fn a_model_selector_prefers_short_names_then_runners() {
        let config: AgentConfig = sc_agent_config::load(
            tempfile::tempdir().expect("dir").path().to_path_buf(),
            Vec::new(),
        )
        .expect("default config");
        assert_eq!(
            ModelSelector::parse("haiku", &config).expect("short"),
            ModelSelector::Short(ShortName::Haiku)
        );
        assert!(matches!(
            ModelSelector::parse("claude-haiku-5-5", &config),
            Ok(ModelSelector::Version(_))
        ));
        assert_eq!(
            ModelSelector::parse("", &config).map_err(|error| error.id()),
            Err("decidr.types")
        );
    }

    /// The timeouts of the UreqTransport `decision_transport` would build, if it builds one.
    fn ureq_timeouts(choice: TransportChoice) -> Option<Timeouts> {
        match choice {
            TransportChoice::Ureq(timeouts) => Some(timeouts),
            TransportChoice::Injected(_) => None,
        }
    }

    /// AC: REQ-SAG-0015 (one transport per local runner with the runner's request timeout;
    /// 300 s when the entry does not set one). Asserts what the real `decision_transport`
    /// builds.
    #[test]
    fn a_local_runner_transport_uses_the_runner_request_timeout() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(
            dir.path().join("default.json"),
            r#"{"local_runners":{
                "laya":{"kind":"laya","host":"127.0.0.1","port":9000,"model":"m"},
                "slow":{"kind":"mapika","host":"127.0.0.1","port":9001,"model":"m",
                        "request_timeout_secs":900,"load_timeout_secs":1200}}}"#,
        )
        .expect("write");
        let config = sc_agent_config::load(dir.path().to_path_buf(), Vec::new()).expect("config");
        let context = Context::new("decidr");
        let laya = local_runner(&config, "laya").expect("laya");
        assert_eq!(
            ureq_timeouts(decision_transport(&context, Some(laya), None)),
            Some(Timeouts::default().with_request(std::time::Duration::from_secs(300)))
        );
        assert_eq!(laya.load_timeout().as_secs(), 600);
        let slow = local_runner(&config, "slow").expect("slow");
        assert_eq!(
            ureq_timeouts(decision_transport(&context, Some(slow), None)),
            Some(Timeouts::default().with_request(std::time::Duration::from_secs(900)))
        );
        assert_eq!(slow.load_timeout().as_secs(), 1200);
        assert_eq!(
            ureq_timeouts(decision_transport(&context, None, None)),
            Some(Timeouts::default())
        );
        let mut injected = Context::new("decidr");
        injected.transport = Some(Arc::new(UreqTransport::new(Timeouts::probe())));
        assert_eq!(
            ureq_timeouts(decision_transport(&injected, Some(laya), None)),
            None
        );
        let timeout = RequestTimeout::from_secs(30.5).expect("checked timeout");
        for selected in [None, Some(laya), Some(slow)] {
            assert_eq!(
                ureq_timeouts(decision_transport(&context, selected, Some(timeout))),
                Some(Timeouts::default().with_request(std::time::Duration::from_millis(30_500))),
            );
        }
        assert_eq!(
            ureq_timeouts(decision_transport(&injected, Some(laya), Some(timeout))),
            None,
            "a caller-owned injected transport keeps its controls",
        );
        let request = crate::input::build_request(
            br#"{"model":"m","state":"s","questions":{"q":{"type":"noul"}}}"#,
            &[],
            false,
            None,
        )
        .expect("request");
        let local = local_backend(laya, &context, Some(timeout)).expect("local backend");
        assert_eq!(
            local
                .encode(&request)
                .expect("encode first request")
                .deadline(),
            Some(laya.load_timeout() + std::time::Duration::from_millis(30_500)),
            "first-request loading retains its configured load allowance",
        );
        assert_eq!(laya.request_timeout(), std::time::Duration::from_secs(300));
        assert_eq!(laya.load_timeout(), std::time::Duration::from_secs(600));
        let error = local_runner(&config, "nope").expect_err("unknown");
        assert_eq!(error.id(), "decidr.unknown_runner");
    }

    /// REQ-SAG-0015 (QA-006): the runner's configured `load_timeout_secs` is the timeout that
    /// reaches `LocalProbe::wait_until_loaded` (recorded by the crate-private seam); the
    /// daemon reports the model loaded at once, so nothing waits.
    #[test]
    fn the_configured_load_timeout_reaches_the_load_wait() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(
            dir.path().join("default.json"),
            r#"{"local_runners":{"dec":{"kind":"mapika","host":"127.0.0.1","port":9100,"model":"jev-latest","load_timeout_secs":17}}}"#,
        )
        .expect("write");
        let transport = sc_agent_http::testing::ScriptedTransport::responses(vec![
            sc_agent_http::HttpResponse::new(200, vec![], br#"{"ok":true,"model":"jev-latest"}"#.to_vec()),
            sc_agent_http::HttpResponse::new(
                200,
                vec![],
                br#"{"model":"jev-latest","answers":{"q":{"type":"noul","noul":0.5}},"usage":{"input_tokens":1,"output_tokens":1}}"#
                    .to_vec(),
            ),
        ]);
        let log = sc_agent_log::testing::MemoryLog::new().expect("log");
        let mut context = Context::new("decidr");
        context.config_dir = Some(dir.path().to_path_buf());
        context.repo_root = Some(dir.path().to_path_buf());
        context.environment = Some(Vec::new());
        context.transport = Some(Arc::new(transport));
        context.general = Some(log.general());
        let config = load_config(&context).expect("config");
        let identity =
            SessionIdentity::new(Repo::local("host", "folder").expect("repo"), "proj", "role")
                .expect("identity");
        let options = SendOptions {
            model: ModelSelector::parse("dec", &config).expect("runner"),
            provider: None,
            project: Slug::new("project", "proj").expect("slug"),
            agent_role: Slug::new("agent_role", "role").expect("slug"),
            context,
            session: Some(log.session(identity)),
        };
        LOAD_TIMEOUT_PASSED.with(|passed| passed.set(None));
        let request = crate::input::build_request(
            br#"{"model":"jev-latest","state":"s","questions":{"q":{"type":"noul"}}}"#,
            &[],
            false,
            None,
        )
        .expect("request");
        let mut delivered = 0;
        send(&options, request, &CancelToken::new(), &mut |result| {
            assert!(result.outcome.is_ok(), "{:?}", result.outcome);
            delivered += 1;
            Ok(())
        })
        .expect("send");
        assert_eq!(delivered, 1);
        assert_eq!(
            LOAD_TIMEOUT_PASSED.with(std::cell::Cell::get),
            Some(std::time::Duration::from_secs(17)),
            "the runner's load_timeout_secs reached the wait"
        );
    }
}

#[cfg(test)]
mod tests {

    use std::cell::RefCell;
    use std::ffi::OsString;
    use std::sync::Arc;
    use std::time::Duration;

    use sc_agent_http::testing::ScriptedTransport;
    use sc_agent_http::{HttpResponse, Timeouts, Transport};
    use sc_agent_log::testing::MemoryLog;
    use sc_agent_log::{Repo, SessionIdentity, Slug};
    use sc_agent_types::CancelToken;

    use super::{Context, ModelSelector, SendOptions};
    use crate::{RequestTimeout, send_batch_with_timeout, send_with_timeout};

    const REQUEST: &[u8] =
        br#"{"model":"jev-latest","state":"s","questions":{"q":{"type":"noul"}}}"#;
    const RESPONSE: &[u8] = br#"{"model":"jev-latest","answers":{"q":{"type":"noul","noul":0.25}},"usage":{"input_tokens":1,"output_tokens":1}}"#;

    thread_local! {
        // Scoped to the calling test thread. Production has no constructor substitution.
        static CONSTRUCTION: RefCell<Option<(ScriptedTransport, Vec<Timeouts>)>> = const { RefCell::new(None) };
    }

    /// The default-transport constructor's controlled test substitute. The public path
    /// selects the timeouts; only construction is substituted, leaving backend/runner intact.
    pub(super) fn record_construction(timeouts: Timeouts) -> Option<Arc<dyn Transport>> {
        CONSTRUCTION.with(|slot| {
            let mut slot = slot.borrow_mut();
            let (transport, seen) = slot.as_mut()?;
            seen.push(timeouts);
            Some(Arc::new(transport.clone()) as Arc<dyn Transport>)
        })
    }

    struct Recording;

    impl Recording {
        fn install(transport: ScriptedTransport) -> Self {
            CONSTRUCTION.with(|slot| {
                assert!(
                    slot.borrow().is_none(),
                    "no nested constructor substitutions"
                );
                *slot.borrow_mut() = Some((transport, Vec::new()));
            });
            Self
        }

        fn timeouts(&self) -> Vec<Timeouts> {
            CONSTRUCTION.with(|slot| slot.borrow().as_ref().expect("installed").1.clone())
        }
    }

    impl Drop for Recording {
        fn drop(&mut self) {
            CONSTRUCTION.with(|slot| *slot.borrow_mut() = None);
        }
    }

    fn options(
        local: bool,
        transport: &ScriptedTransport,
        log: &MemoryLog,
    ) -> (tempfile::TempDir, SendOptions) {
        let dir = tempfile::tempdir().expect("config dir");
        std::fs::write(
            dir.path().join("default.json"),
            if local {
                r#"{"local_runners":{"laya":{"kind":"laya","host":"127.0.0.1","port":9000,"model":"jev-latest","request_timeout_secs":900,"load_timeout_secs":17}}}"#
            } else {
                "{}"
            },
        ).expect("write config");
        let mut context = Context::new("decidr");
        context.config_dir = Some(dir.path().to_owned());
        context.repo_root = Some(dir.path().to_owned());
        context.environment = Some(vec![(
            OsString::from(sc_agent_secrets::TYPESAFE_API_KEY),
            OsString::from("timeout-test-key"),
        )]);
        context.general = Some(log.general());
        context.clock = Some(sc_agent_http::testing::ManualClock::new().clock());
        if local {
            context.transport = Some(Arc::new(transport.clone()));
        }
        let config = super::load_config(&context).expect("config");
        let model =
            ModelSelector::parse(if local { "laya" } else { "jev" }, &config).expect("model");
        let identity =
            SessionIdentity::new(Repo::local("host", "folder").expect("repo"), "proj", "role")
                .expect("identity");
        (
            dir,
            SendOptions {
                model,
                provider: None,
                project: Slug::new("project", "proj").expect("project"),
                agent_role: Slug::new("agent_role", "role").expect("role"),
                context,
                session: Some(log.session(identity)),
            },
        )
    }

    fn run(batch: bool, options: &SendOptions, timeout: Option<RequestTimeout>) {
        let request = crate::input::build_request(REQUEST, &[], false, None).expect("request");
        let mut count = 0;
        let mut delivered = |result: sc_decision_schema::DecisionResult| {
            assert!(result.outcome.is_ok(), "{:?}", result.outcome);
            count += 1;
            Ok(())
        };
        if batch {
            send_batch_with_timeout(
                options,
                vec![request],
                &CancelToken::new(),
                timeout,
                &mut delivered,
            )
        } else {
            send_with_timeout(
                options,
                request,
                &CancelToken::new(),
                timeout,
                &mut delivered,
            )
        }
        .expect("public send");
        assert_eq!(
            count, 1,
            "one successful decision reached the public callback"
        );
    }

    fn hosted(batch: bool) {
        for seconds in [None, Some(30.5)] {
            let transport = ScriptedTransport::responses(vec![HttpResponse::new(
                200,
                vec![],
                RESPONSE.to_vec(),
            )]);
            let recording = Recording::install(transport.clone());
            let log = MemoryLog::new().expect("log");
            let (_dir, options) = options(false, &transport, &log);
            let timeout = seconds.map(|value| RequestTimeout::from_secs(value).expect("timeout"));
            run(batch, &options, timeout);
            assert_eq!(
                recording.timeouts(),
                [Timeouts {
                    connect: Duration::from_secs(10),
                    request: Duration::from_secs_f64(seconds.unwrap_or(120.0)),
                }],
                "the hosted default transport constructor received the per-call value"
            );
            let requests = transport.requests();
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].endpoint().host(), "api.typesafe.ai");
        }
    }

    fn local(batch: bool) {
        for seconds in [None, Some(30.5)] {
            let transport = ScriptedTransport::responses(vec![
                HttpResponse::new(200, vec![], br#"{"status":"ok","loaded":[]}"#.to_vec()),
                HttpResponse::new(200, vec![], RESPONSE.to_vec()),
            ]);
            let log = MemoryLog::new().expect("log");
            let (_dir, options) = options(true, &transport, &log);
            let timeout = seconds.map(|value| RequestTimeout::from_secs(value).expect("timeout"));
            run(batch, &options, timeout);
            let requests = transport.requests();
            assert_eq!(requests.len(), 2, "one health probe, then one decision");
            assert_eq!(requests[0].deadline(), None, "the probe was not overridden");
            assert!(requests[1].endpoint().as_str().ends_with("/v1/systemone"));
            assert_eq!(
                requests[1].deadline(),
                Some(Duration::from_secs(17) + Duration::from_secs_f64(seconds.unwrap_or(900.0))),
                "the decision transport received configured load allowance + per-call request timeout"
            );
        }
    }

    #[test]
    fn public_send_delivers_hosted_timeout_to_transport() {
        hosted(false);
    }

    #[test]
    fn public_batch_delivers_hosted_timeout_to_transport() {
        hosted(true);
    }

    #[test]
    fn public_send_delivers_local_timeout_to_transport() {
        local(false);
    }

    #[test]
    fn public_batch_delivers_local_timeout_to_transport() {
        local(true);
    }
}
