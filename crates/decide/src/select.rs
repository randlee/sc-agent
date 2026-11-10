//! The one place a backend is chosen and the runner is wired (ADR-SAG-0004 composition root).
//!
//! [`send`] and [`send_batch`] load the configuration, choose the backend (a configured local
//! runner, TypeSafe, or an LLM provider by ranking), build one `HttpClient` and one
//! `DecisionRunner`, and run the requests through the `Decision` trait. [`models`] lists the
//! TypeSafe or runner models. Every seam of [`Context`] is optional (REQ-SAG-0008).

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use sc_agent_config::{AgentConfig, LocalRunner};
use sc_agent_http::{
    Attempts, HttpClient, HttpError, HttpRequest, HttpResponse, LogContext, Next, Pending,
    ResendPolicy, Start, Timeouts, Transport, UreqTransport,
};
use sc_agent_log::{GeneralLogger, Level, LogError, Repo, SessionIdentity, SessionLogger, Slug};
use sc_agent_secrets::{
    EnvSource, KeySources, KeyringSource, PlatformKeyring, ResolvedKey, SecretsError, resolve,
    resolve_var,
};
use sc_agent_types::{
    CancelToken, ErrorId, ModelChoice, ModelId, Provider, SessionId, ShortName, Wait,
};
use sc_decision_llm::LlmDecisionBackend;
use sc_decision_local::{DaemonKind, LocalBackend, LocalTimeouts};
use sc_decision_runner::{DecisionRunner, RunnerConfig};
use sc_decision_schema::{
    Backend, Decision, DecisionError, DecisionRequest, DecisionResult, ModelMetadataList,
};
use sc_decision_systemone::{SystemOneBackend, TYPESAFE_BASE_URL};
use sc_llm_provider::{LlmClient, LlmProvider};
use serde_json::{Map, Value};

use crate::DecideError;

/// The general-log target of decide's own records.
pub(crate) const LOG_TARGET: &str = "decide";

/// Shared seams of every decide operation; every field `None` is the real default
/// (REQ-SAG-0008).
#[derive(Clone)]
pub struct Context {
    /// The general-log service: `"decide"` or `"sc-agent-py"`.
    pub service: &'static str,
    /// The configuration directory; `None` is `sc_agent_config::default_dir()`.
    pub config_dir: Option<PathBuf>,
    /// `SC_AGENT__` overrides and key lookup; `None` is `std::env::vars_os()`.
    pub environment: Option<Vec<(OsString, OsString)>>,
    /// The keyring read last for keys; `None` is the platform keyring.
    pub keyring: Option<Arc<dyn KeyringSource + Send + Sync>>,
    /// Every send; `None` is a `UreqTransport` with the operation's timeouts.
    pub transport: Option<Arc<dyn Transport>>,
    /// The general log; `None` is `GeneralLogger::open_default(log_root, service)`.
    pub general: Option<GeneralLogger>,
}

impl Context {
    /// A context for `service` with every seam `None`.
    #[must_use]
    pub fn new(service: &'static str) -> Self {
        Self {
            service,
            config_dir: None,
            environment: None,
            keyring: None,
            transport: None,
            general: None,
        }
    }

    fn environment(&self) -> Vec<(OsString, OsString)> {
        self.environment
            .clone()
            .unwrap_or_else(|| std::env::vars_os().collect())
    }
}

impl fmt::Debug for Context {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Context")
            .field("service", &self.service)
            .field("config_dir", &self.config_dir)
            .field("environment", &self.environment.as_ref().map(Vec::len))
            .field("keyring", &self.keyring.is_some())
            .field("transport", &self.transport.is_some())
            .field("general", &self.general.is_some())
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
    Runner(String),
}

impl ModelSelector {
    /// A short name if `text` is one; else a configured local runner name; else a model id.
    ///
    /// # Errors
    ///
    /// [`DecideError::Types`] (`types.invalid_model_id`) when `text` is none of them.
    pub fn parse(text: &str, config: &AgentConfig) -> Result<Self, DecideError> {
        if let Ok(short) = text.parse::<ShortName>() {
            return Ok(Self::Short(short));
        }
        if config.local_runners.contains_key(text) {
            return Ok(Self::Runner(text.to_owned()));
        }
        text.parse::<ModelId>()
            .map(Self::Version)
            .map_err(DecideError::Types)
    }
}

impl fmt::Display for ModelSelector {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Short(short) => fmt::Display::fmt(short, formatter),
            Self::Version(model) => fmt::Display::fmt(model, formatter),
            Self::Runner(name) => formatter.write_str(name),
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
    pub runner: Option<String>,
    /// The shared seams.
    pub context: Context,
}

/// Loads the global configuration of `context` (REQ-SAG-0013 bounds included).
///
/// # Errors
///
/// [`DecideError::Config`] with the `AgentConfigError` (key path in `config.invalid`).
pub fn load_config(context: &Context) -> Result<AgentConfig, DecideError> {
    let dir = match &context.config_dir {
        Some(dir) => dir.clone(),
        None => sc_agent_config::default_dir().map_err(DecideError::Config)?,
    };
    sc_agent_config::load(dir, context.environment()).map_err(DecideError::Config)
}

/// The session identity of the working folder: `Repo::detect(cwd)`, `project`, `agent_role`.
///
/// # Errors
///
/// [`DecideError::Log`] when the working folder or its repo cannot be determined.
pub fn session_identity(project: &Slug, agent_role: &Slug) -> Result<SessionIdentity, DecideError> {
    let cwd = working_dir()?;
    let repo = Repo::detect(&cwd).map_err(DecideError::from)?;
    SessionIdentity::new(repo, project.as_str(), agent_role.as_str()).map_err(DecideError::from)
}

fn working_dir() -> Result<PathBuf, DecideError> {
    std::env::current_dir().map_err(|_| {
        DecideError::from(LogError::FolderUnknown {
            path: PathBuf::from("."),
        })
    })
}

/// The caller's environment as an [`EnvSource`].
struct VarsEnv(Vec<(OsString, OsString)>);

impl EnvSource for VarsEnv {
    fn var(&self, name: &str) -> Option<String> {
        self.0
            .iter()
            .rev()
            .find(|(key, _)| key.as_os_str() == name)
            .map(|(_, value)| value.clone().into_string().unwrap_or_default())
    }
}

/// Key lookup sources of one operation: environment, repo-root `.env`, keyring (REQ-SAG-0401).
struct Keys {
    env: VarsEnv,
    repo_root: Option<PathBuf>,
    keyring: Arc<dyn KeyringSource + Send + Sync>,
}

impl Keys {
    fn new(context: &Context) -> Result<Self, DecideError> {
        let cwd = working_dir()?;
        Ok(Self {
            env: VarsEnv(context.environment()),
            repo_root: Repo::work_tree_root(&cwd).map_err(DecideError::from)?,
            keyring: context
                .keyring
                .clone()
                .unwrap_or_else(|| Arc::new(PlatformKeyring)),
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
    fn find(&self, provider: Provider) -> Result<Option<ResolvedKey>, DecideError> {
        match resolve(provider, &self.sources()) {
            Ok(key) => Ok(Some(key)),
            Err(SecretsError::KeyMissing { .. }) => Ok(None),
            Err(error) => Err(DecideError::Secrets(error)),
        }
    }
}

/// The provider of a request and its key: the override, else the first of
/// `Provider::ranking(line)` with a detected key (REQ-SAG-0103).
fn choose_provider(
    choice: Option<Provider>,
    line: Option<ShortName>,
    model: &str,
    keys: &Keys,
) -> Result<(Provider, ResolvedKey), DecideError> {
    if let Some(provider) = choice {
        let key = resolve(provider, &keys.sources()).map_err(DecideError::Secrets)?;
        return Ok((provider, key));
    }
    let ranking = Provider::ranking(line);
    for provider in ranking {
        if let Some(key) = keys.find(*provider)? {
            return Ok((*provider, key));
        }
    }
    Err(DecideError::NoProviderKey {
        model: model.to_owned(),
        tried: ranking.to_vec(),
    })
}

/// The general logger of `context`, or the default one for its service.
fn general_logger(context: &Context, config: &AgentConfig) -> Result<GeneralLogger, DecideError> {
    match &context.general {
        Some(general) => Ok(general.clone()),
        None => {
            let root = config.log_root().map_err(DecideError::Config)?;
            GeneralLogger::open_default(&root, context.service).map_err(DecideError::from)
        }
    }
}

/// The configured runner `name`.
fn local_runner<'c>(config: &'c AgentConfig, name: &str) -> Result<&'c LocalRunner, DecideError> {
    config
        .local_runners
        .get(name)
        .ok_or_else(|| DecideError::UnknownRunner {
            name: name.to_owned(),
            known: config
                .local_runners
                .keys()
                .map(|key| key.as_str().to_owned())
                .collect(),
        })
}

/// The runner's key, when its entry names `key_env`.
fn runner_key(
    runner: &LocalRunner,
    context: &Context,
) -> Result<Option<sc_agent_secrets::ApiKey>, DecideError> {
    let Some(var) = &runner.key_env else {
        return Ok(None);
    };
    let keys = Keys::new(context)?;
    let key = resolve_var(var, &keys.sources()).map_err(DecideError::Secrets)?;
    Ok(Some(key.key))
}

/// A `LocalBackend` for `runner`, with its REQ-SAG-0015 timeouts and the general log.
fn local_backend(
    runner: &LocalRunner,
    context: &Context,
    general: &GeneralLogger,
) -> Result<LocalBackend, DecideError> {
    let key = runner_key(runner, context)?;
    let backend = LocalBackend::new(
        DaemonKind::from(runner.kind),
        &runner.host,
        runner.port,
        runner.model.clone(),
        key,
    )
    .map_err(DecideError::from)?;
    Ok(backend
        .with_log(general.clone())
        .with_timeouts(LocalTimeouts {
            load: runner.load_timeout(),
            request: runner.request_timeout(),
        }))
}

/// The backend a send uses.
enum Chosen {
    /// A configured local runner.
    Local {
        name: String,
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

    /// `(provider, model, runner)` fields of the `decide.send` record.
    fn label(&self) -> (String, String, Option<String>) {
        match self {
            Self::Local { name, runner, .. } => (
                "local".to_owned(),
                runner.model.as_str().to_owned(),
                Some(name.clone()),
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
) -> Result<Chosen, DecideError> {
    let (model, line) = match &options.model {
        ModelSelector::Runner(name) => {
            let runner = local_runner(config, name)?;
            let backend = local_backend(runner, &options.context, general)?;
            return Ok(Chosen::Local {
                name: name.clone(),
                runner: runner.clone(),
                backend: Arc::new(backend),
            });
        }
        ModelSelector::Short(short) => {
            let catalog = config.catalog().map_err(DecideError::Config)?;
            (catalog.resolve(&ModelChoice::Latest(*short)), Some(*short))
        }
        ModelSelector::Version(model) => {
            let catalog = config.catalog().map_err(DecideError::Config)?;
            (model.clone(), catalog.line_of(model))
        }
    };
    let keys = Keys::new(&options.context)?;
    let (provider, key) =
        choose_provider(options.provider, line, &options.model.to_string(), &keys)?;
    let backend: Arc<dyn Backend> = match LlmProvider::from_provider(provider) {
        None => Arc::new(
            SystemOneBackend::new(TYPESAFE_BASE_URL, Some(key.key)).map_err(DecideError::from)?,
        ),
        Some(llm) => {
            let client = LlmClient::new(llm, key.key)
                .map_err(DecideError::Llm)?
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

/// The decision transport: the context's, else one `UreqTransport` per local runner with the
/// runner's request timeout (REQ-SAG-0015), else the default timeouts.
fn decision_transport(context: &Context, runner: Option<&LocalRunner>) -> Arc<dyn Transport> {
    if let Some(transport) = &context.transport {
        return Arc::clone(transport);
    }
    Arc::new(UreqTransport::new(decision_timeouts(runner)))
}

/// `Timeouts::default()`, with a local runner's own request timeout (REQ-SAG-0015).
fn decision_timeouts(runner: Option<&LocalRunner>) -> Timeouts {
    match runner {
        Some(runner) => Timeouts::default().with_request(runner.request_timeout()),
        None => Timeouts::default(),
    }
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
    config: &AgentConfig,
    transport: Arc<dyn Transport>,
    session: SessionLogger,
    general: GeneralLogger,
    backend: Arc<dyn Backend>,
) -> DecisionRunner {
    let http = HttpClient::new(
        transport,
        ResendPolicy::new(config.max_sends),
        general.clone(),
    );
    DecisionRunner::new(
        backend,
        http,
        session,
        general,
        RunnerConfig::from_config(config),
    )
}

/// Everything one send or batch needs, built before anything is sent.
struct Prepared {
    runner: DecisionRunner,
    general: GeneralLogger,
    chosen: Chosen,
    probe: Arc<dyn Transport>,
    service: &'static str,
}

/// Whether the local model is loaded, or the wait was cancelled.
enum Loaded {
    Ready,
    Cancelled,
}

impl Prepared {
    fn new(options: &SendOptions) -> Result<Self, DecideError> {
        let context = &options.context;
        let config = load_config(context)?;
        let general = general_logger(context, &config)?;
        let chosen = backend(options, &config, &general)?;
        let session = match &options.session {
            Some(session) => session.clone(),
            None => {
                let identity = session_identity(&options.project, &options.agent_role)?;
                let root = config.log_root().map_err(DecideError::Config)?;
                SessionLogger::open_default(&root, identity).map_err(DecideError::from)?
            }
        };
        let local = match &chosen {
            Chosen::Local { runner, .. } => Some(runner),
            Chosen::Remote { .. } => None,
        };
        let transport = decision_transport(context, local);
        let runner = runner(
            &config,
            transport,
            session,
            general.clone(),
            chosen.backend(),
        );
        Ok(Self {
            runner,
            general,
            chosen,
            probe: probe_transport(context),
            service: context.service,
        })
    }

    /// For a local runner, waits until its model is loaded before the first send
    /// (REQ-SAG-0015), so load time never counts toward a request timeout or the 90 s budget.
    fn wait_loaded(&self, cancel: &CancelToken) -> Result<Loaded, DecideError> {
        let Chosen::Local {
            runner, backend, ..
        } = &self.chosen
        else {
            return Ok(Loaded::Ready);
        };
        match backend.wait_until_loaded(
            &*self.probe,
            cancel,
            runner.load_timeout(),
            LogContext::NONE,
        ) {
            Ok(()) => Ok(Loaded::Ready),
            Err(DecisionError::Cancelled { .. }) => Ok(Loaded::Cancelled),
            Err(error) => Err(DecideError::from(error)),
        }
    }

    /// Writes the one `decide.send` record of this operation (never a key).
    fn record(&self, tally: &Tally, error: Option<&DecideError>) {
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
            "decide.send",
            LogContext::NONE,
            fields,
        );
    }
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

/// Sends one request: always `Decision::decide` (CRIT-M3), then `flush_session_log`, and one
/// `decide.send` record.
///
/// A failed decision is data in the returned result.
///
/// # Errors
///
/// Before any send: `Config`, `Types`, `Secrets`, `Log`, `Local`, `Llm`, `Decision` (a local
/// runner that does not load), `NoProviderKey`, `UnknownRunner`. After the send:
/// [`DecideError::Runner`] when the session log cannot be flushed.
pub fn send(
    options: &SendOptions,
    request: DecisionRequest,
    cancel: &CancelToken,
) -> Result<DecisionResult, DecideError> {
    let prepared = Prepared::new(options)?;
    let mut tally = Tally::default();
    let loaded = match prepared.wait_loaded(cancel) {
        Ok(loaded) => loaded,
        Err(error) => {
            prepared.record(&tally, Some(&error));
            return Err(error);
        }
    };
    let result = match loaded {
        Loaded::Ready => send_on(&prepared.runner, request, cancel),
        Loaded::Cancelled => cancelled_before_send(&request, 0),
    };
    tally.add(&result);
    let flushed = prepared
        .runner
        .flush_session_log()
        .map_err(DecideError::from);
    prepared.record(&tally, flushed.as_ref().err());
    flushed?;
    Ok(result)
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
/// [`DecideError::Runner`] when the session log cannot be flushed.
pub fn send_batch(
    options: &SendOptions,
    requests: Vec<DecisionRequest>,
    cancel: &CancelToken,
    on_result: &mut dyn FnMut(DecisionResult) -> Result<(), DecideError>,
) -> Result<(), DecideError> {
    let prepared = Prepared::new(options)?;
    let mut tally = Tally::default();
    let loaded = match prepared.wait_loaded(cancel) {
        Ok(loaded) => loaded,
        Err(error) => {
            prepared.record(&tally, Some(&error));
            return Err(error);
        }
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
        .map_err(DecideError::from);
    let result = outcome.and(flushed);
    prepared.record(&tally, result.as_ref().err());
    result
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
    on_result: &mut dyn FnMut(DecisionResult) -> Result<(), DecideError>,
) -> Result<(), DecideError> {
    let stream = decision.decide_stream(Box::new(requests.into_iter()), cancel);
    deliver(stream, cancel, on_result)
}

/// Hands each result to `on_result`; after its first error, cancels and drains the rest.
fn deliver(
    results: impl Iterator<Item = DecisionResult>,
    cancel: &CancelToken,
    on_result: &mut dyn FnMut(DecisionResult) -> Result<(), DecideError>,
) -> Result<(), DecideError> {
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
    cancel: &CancelToken,
) -> Result<HttpResponse, DecideError> {
    let cancelled = |sends: u32| DecideError::from(DecisionError::Cancelled { sends });
    let mut pending: Option<Pending> = None;
    loop {
        if cancel.is_cancelled() {
            return Err(cancelled(
                pending.as_ref().map_or(0, |held| held.attempts().sends()),
            ));
        }
        let now = Instant::now();
        let permit = match pending.take() {
            None => match http.may_start(request, Attempts::new(), now) {
                Ok(Start::Go(permit)) => permit,
                Ok(Start::Wait { at, .. }) => {
                    wait(cancel, at, 0)?;
                    continue;
                }
                Err(reason) => {
                    return Err(DecideError::Http(HttpError::invalid_request(format!(
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
                    wait(cancel, at, sends)?;
                    continue;
                }
                Err(error) => return Err(DecideError::Http(error)),
            },
        };
        match http
            .send_once(request, permit, LogContext::NONE, &Instant::now)
            .map_err(DecideError::Http)?
        {
            Next::Done { response, .. } => return Ok(response),
            Next::ResendAt { at, pending: held } => {
                let sends = held.attempts().sends();
                pending = Some(held);
                wait(cancel, at, sends)?;
            }
        }
    }
}

/// Waits until `at` or a cancel.
fn wait(cancel: &CancelToken, at: Instant, sends: u32) -> Result<(), DecideError> {
    match cancel.wait_until(at) {
        Wait::Cancelled => Err(DecideError::from(DecisionError::Cancelled { sends })),
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
    cancel: &CancelToken,
) -> Result<HttpResponse, DecideError> {
    send_probe(http, request, cancel)
}

/// The TypeSafe `SystemOneBackend` with its resolved key.
pub(crate) fn typesafe_backend(key: ResolvedKey) -> Result<SystemOneBackend, DecideError> {
    SystemOneBackend::new(TYPESAFE_BASE_URL, Some(key.key)).map_err(DecideError::from)
}

/// Key lookup for doctor: every provider in `Provider::PRIORITY` order with a key.
pub(crate) fn detected_keys(
    context: &Context,
) -> Result<Vec<(Provider, ResolvedKey)>, DecideError> {
    let keys = Keys::new(context)?;
    let mut found = Vec::new();
    for provider in Provider::PRIORITY {
        if let Some(key) = keys.find(provider)? {
            found.push((provider, key));
        }
    }
    Ok(found)
}

/// The general logger and config of a probe-class operation.
pub(crate) fn probe_setup(context: &Context) -> Result<(AgentConfig, GeneralLogger), DecideError> {
    let config = load_config(context)?;
    let general = general_logger(context, &config)?;
    Ok((config, general))
}

/// GET /v1/models of TypeSafe or a configured runner, sent once with `Timeouts::probe()`
/// (A-5); writes one `decide.models` record.
///
/// # Errors
///
/// `Config`, `Log`, `UnknownRunner`, `Secrets` (TypeSafe key), `Local`, `Decision` (the
/// request or the reply), `Http` (the send).
pub fn models(
    options: &ModelsOptions,
    cancel: &CancelToken,
) -> Result<ModelMetadataList, DecideError> {
    let (config, general) = probe_setup(&options.context)?;
    let result = list_models(options, &config, &general, cancel);
    let mut fields = Map::new();
    fields.insert("service".into(), options.context.service.into());
    fields.insert(
        "runner".into(),
        options.runner.as_deref().unwrap_or("typesafe").into(),
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
        "decide.models",
        LogContext::NONE,
        fields,
    );
    result
}

fn list_models(
    options: &ModelsOptions,
    config: &AgentConfig,
    general: &GeneralLogger,
    cancel: &CancelToken,
) -> Result<ModelMetadataList, DecideError> {
    let backend = match &options.runner {
        Some(name) => {
            let runner = local_runner(config, name)?;
            let key = runner_key(runner, &options.context)?;
            let base = LocalBackend::new(
                DaemonKind::from(runner.kind),
                &runner.host,
                runner.port,
                runner.model.clone(),
                None,
            )
            .map_err(DecideError::from)?
            .base()
            .to_owned();
            SystemOneBackend::local(&base, key).map_err(DecideError::from)?
        }
        None => {
            let keys = Keys::new(&options.context)?;
            let key = resolve(Provider::TypeSafe, &keys.sources()).map_err(DecideError::Secrets)?;
            typesafe_backend(key)?
        }
    };
    let request = backend.models_request().map_err(DecideError::from)?;
    let http = probe_client(&options.context, config, general);
    let response = probe(&http, &request, cancel)?;
    backend.decode_models(response).map_err(DecideError::from)
}

/// The local-runner request timeout decide gives the runner's transport (REQ-SAG-0015).
#[cfg(test)]
pub(crate) fn runner_request_timeout(runner: &LocalRunner) -> std::time::Duration {
    decision_timeouts(Some(runner)).request
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoKeyring;

    impl KeyringSource for NoKeyring {
        fn password(&self, _: &str, _: &str) -> Result<Option<String>, SecretsError> {
            Ok(None)
        }
    }

    fn keys(vars: &[(&str, &str)]) -> Keys {
        Keys {
            env: VarsEnv(
                vars.iter()
                    .map(|(name, value)| (OsString::from(name), OsString::from(value)))
                    .collect(),
            ),
            repo_root: None,
            keyring: Arc::new(NoKeyring),
        }
    }

    /// AC: REQ-SAG-0103 (provider choice cases).
    #[test]
    fn the_first_ranked_provider_with_a_key_is_chosen() {
        let both = keys(&[
            ("ANTHROPIC_API_KEY", "sk-a"),
            ("OPENROUTER_API_KEY", "sk-o"),
        ]);
        let haiku = Some(ShortName::Haiku);
        let (provider, _) = choose_provider(None, haiku, "haiku", &both).expect("anthropic");
        assert_eq!(provider, Provider::Anthropic);

        let reseller = keys(&[("OPENROUTER_API_KEY", "sk-o")]);
        let (provider, _) = choose_provider(None, haiku, "haiku", &reseller).expect("openrouter");
        assert_eq!(provider, Provider::OpenRouter);

        let (provider, _) =
            choose_provider(Some(Provider::OpenRouter), haiku, "haiku", &both).expect("override");
        assert_eq!(provider, Provider::OpenRouter);

        let error = choose_provider(None, Some(ShortName::Jev), "jev", &both).expect_err("no key");
        assert_eq!(error.id(), "decide.no_provider_key");
        assert!(error.recovery().is_some());
        assert!(matches!(
            error,
            DecideError::NoProviderKey { ref tried, .. } if tried == &[Provider::TypeSafe]
        ));
    }

    #[test]
    fn an_override_without_its_key_is_a_secrets_error() {
        let none = keys(&[]);
        let error = choose_provider(Some(Provider::OpenAi), Some(ShortName::Luna), "luna", &none)
            .expect_err("no key");
        assert_eq!(error.id(), "decide.secrets");
        assert_eq!(
            error.cause_detail().map(|cause| cause.id),
            Some("secrets.key_missing".to_owned())
        );
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
            Err("decide.types")
        );
    }

    /// AC: REQ-SAG-0015 (one transport per local runner with the runner's request timeout;
    /// 300 s when the entry does not set one).
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
        let laya = local_runner(&config, "laya").expect("laya");
        assert_eq!(runner_request_timeout(laya).as_secs(), 300);
        assert_eq!(laya.load_timeout().as_secs(), 600);
        let slow = local_runner(&config, "slow").expect("slow");
        assert_eq!(runner_request_timeout(slow).as_secs(), 900);
        assert_eq!(slow.load_timeout().as_secs(), 1200);
        assert_eq!(decision_timeouts(None), Timeouts::default());
        let error = local_runner(&config, "nope").expect_err("unknown");
        assert_eq!(error.id(), "decide.unknown_runner");
    }
}
