use sc_config::{
    AppName, ConfigError, EnvSource, FileSource, Layer, LayerSource, Resolved, Slot, SourceKind,
    StandardContext, StandardLayers,
};
use sc_observability::{LoggerConfig, ServiceName, v2::Logger};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Default)]
pub struct Cancel {
    state: Mutex<bool>,
    changed: Condvar,
}
impl Cancel {
    pub fn cancel(&self) {
        if let Ok(mut state) = self.state.lock() {
            *state = true;
        }
        self.changed.notify_all();
    }
    pub fn is_cancelled(&self) -> bool {
        self.state.lock().map_or(true, |state| *state)
    }
    pub fn wait(&self, delay: Duration) {
        if let Ok(state) = self.state.lock() {
            let _ = self
                .changed
                .wait_timeout_while(state, delay, |state| !*state);
        }
    }
}

impl Cancel {
    pub fn check(&self) -> Result<(), Error> {
        if self.is_cancelled() {
            Err(error("decision.cancelled", "request cancelled"))
        } else {
            Ok(())
        }
    }
    pub fn run<T: Send + 'static>(
        &self,
        f: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T, Error> {
        self.check()?;
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let _ = sender.send(f());
        });
        loop {
            self.check()?;
            match receiver.recv_timeout(Duration::from_millis(20)) {
                Ok(value) => return Ok(value),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(e) => return Err(error("http.network", e)),
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, thiserror::Error)]
#[serde(untagged)]
#[error("{id}: {message}")]
pub enum Error {
    Failure {
        id: &'static str,
        message: String,
        action: &'static str,
        category: &'static str,
        #[serde(skip_serializing_if = "Option::is_none")]
        attempts: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
}
pub fn error(id: &'static str, message: impl ToString) -> Error {
    let category = match id {
        "secrets.key_missing"
        | "secrets.key_malformed"
        | "secrets.keyring_read"
        | "secrets.keyring_timeout"
        | "http.auth" => "auth",
        "http.network" => "network",
        "http.provider_busy" => "busy",
        "http.quota_exhausted" => "quota",
        "decision.cancelled" => "cancelled",
        "decision.response_parse" | "decision.schema_mismatch" | "decision.model_mismatch" => {
            "response_invalid"
        }
        s if s.starts_with("config.") => "config",
        s if s.starts_with("log.") && s != "log.invalid_slug" => "log",
        "decision.api_status" => "other",
        _ => "validation",
    };
    Error::Failure {
        id,
        message: message.to_string(),
        category,
        action: match id {
            "decision.run_closed" => "Create a new Run; a closed Run accepts no calls.",
            "decision.model_mismatch" => {
                "Point `<endpoint>.host`/`.port` at the server that runs the selected model, or select the model that server runs."
            }
            "types.unknown_short_name" => {
                "Use an alias or id listed by `decidr models`, or add a row with `models.<alias>` (`provider`, `id`)."
            }
            "config.invalid" => {
                "Fix or remove the named key in the named file, or unset the named environment variable (`models` is set in files only)."
            }
            _ => match category {
                "auth" if id == "secrets.keyring_timeout" => {
                    "Approve keychain access, or set the environment variable or .env key."
                }
                "auth" if id == "secrets.keyring_read" => {
                    "Restore platform keyring access, or set the key in the environment or repository .env."
                }
                "auth" if id == "secrets.key_missing" => {
                    "Set the key in the environment, repository .env, or platform keyring."
                }
                "auth" => "Check the provider API key.",
                "busy" | "network" => "Retry later.",
                "quota" | "other" => "Check the provider response.",
                _ => "Fix the named input or provider response.",
            },
        },
        attempts: None,
        session_id: None,
    }
}
impl Error {
    pub fn attempts(mut self, n: u32) -> Self {
        let Self::Failure { attempts, .. } = &mut self;
        *attempts = Some(n);
        self
    }
    pub fn id(&self) -> &'static str {
        let Self::Failure { id, .. } = self;
        id
    }
    pub fn category(&self) -> &'static str {
        let Self::Failure { category, .. } = self;
        category
    }
}
#[derive(Clone, Default)]
pub struct Secret<T>(T);
impl<T> std::fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[redacted]")
    }
}
impl<T> Secret<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }
    pub fn expose(&self) -> &T {
        &self.0
    }
}
#[derive(Clone, Debug, Default)]
pub struct Environment(pub Secret<Vec<(OsString, OsString)>>);
impl Environment {
    pub fn process() -> Self {
        Self(Secret::new(std::env::vars_os().collect()))
    }
    pub fn log_root(&self, name: &str) -> Result<PathBuf, Error> {
        self.0
            .expose()
            .iter()
            .find(|(key, value)| {
                key == format!("{}_LOG_ROOT", name.to_uppercase().replace('-', "_")).as_str()
                    && !value.is_empty()
            })
            .map(|(_, value)| PathBuf::from(value))
            .or_else(|| std::env::home_dir().map(|p| p.join(".sc-agent")))
            .ok_or_else(|| error("config.home", "Home directory unavailable"))
    }
}
pub mod providers;
use providers::{Base, KeySources, ResolvedModel, Wire};
pub fn result(value: Result<Value, Error>) -> Value {
    match value {
        Ok(value) => json!({"ok":true,"value":value}),
        Err(error) => json!({"ok":false,"error":error}),
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelEntry {
    pub provider: String,
    pub id: String,
    #[serde(default)]
    pub released: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct ConfiguredRow {
    pub id: String,
    pub released: Option<String>,
    pub provider: &'static providers::Provider,
}
#[derive(Clone, Debug, Serialize)]
pub struct Settings {
    pub project: Option<String>,
    pub workers: usize,
    pub model: String,
    pub local: BTreeMap<&'static str, Endpoint>,
    pub models: BTreeMap<String, ConfiguredRow>,
}
#[derive(Clone, Debug)]
pub struct Repository {
    pub root: PathBuf,
    pub identity: String,
}
impl Repository {
    pub fn local_config(&self) -> Option<PathBuf> {
        self.root
            .join(".git")
            .exists()
            .then(|| self.root.join(".sc/sc-agent"))
    }
}
pub struct ConfigSources<'a> {
    pub local: Option<PathBuf>,
    pub global: Option<PathBuf>,
    pub environment: &'a Environment,
    pub project: Option<&'a str>,
}
impl<'a> ConfigSources<'a> {
    pub fn new(
        repository: &Repository,
        config_dir: Option<&Path>,
        environment: &'a Environment,
        project: Option<&'a str>,
    ) -> Self {
        Self {
            local: repository.local_config(),
            global: config_dir.map(Path::to_path_buf),
            environment,
            project,
        }
    }
}
fn missing_setting(path: &str) -> Error {
    error(
        "config.missing_setting",
        format!(
            "Set {path} through DECIDR_{}, local decidr.local.json, local decidr.json, local sc-agent.json, global decidr.local.json, global decidr.json, or global sc-agent.json",
            path.to_uppercase().replace('.', "__")
        ),
    )
}
fn setting<T: serde::de::DeserializeOwned>(
    resolved: &Resolved,
    path: &str,
    default: Value,
) -> Result<T, Error> {
    let selected = resolved
        .value
        .pointer(&format!("/{}", path.replace('.', "/")));

    serde_json::from_value(selected.cloned().unwrap_or(default))
        .map_err(|e| invalid_setting(resolved, path, e))
}
impl Settings {
    pub fn project_required(&self) -> Result<&str, Error> {
        self.project.as_deref().filter(|s| !s.is_empty()).ok_or_else(||error("config.missing_project", "Set project/phase through --project, SC_PROJECT, local decidr.local.json, local decidr.json, local sc-agent.json, global decidr.local.json, global decidr.json, or global sc-agent.json"))
    }
}
fn config_keys() -> Vec<String> {
    ["project", "workers", "model", "models"]
        .into_iter()
        .map(str::to_owned)
        .chain(
            providers::PROVIDERS
                .iter()
                .filter_map(|p| match p.base {
                    Base::Local(name) => Some(name),
                    _ => None,
                })
                .flat_map(|n| [format!("{n}.host"), format!("{n}.port")]),
        )
        .collect()
}
pub struct Configuration {
    pub config: Settings,
    pub rows: Vec<Value>,
    pub files: Vec<Value>,
}
impl LayerSource for Environment {
    fn load(&self, base: &Value) -> Result<Option<Layer>, ConfigError> {
        let variables = self
            .0
            .expose()
            .iter()
            .filter(|(name, _)| {
                let key = name
                    .to_str()
                    .and_then(|name| name.strip_prefix("DECIDR_"))
                    .and_then(|key| key.split("__").next())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                !matches!(key.as_str(), "log_root" | "project")
            })
            .cloned()
            .collect();
        EnvSource::new("DECIDR_", variables)
            .alias("SC_PROJECT", ["project"])
            .load(base)
    }
}
fn config_error(failure: ConfigError) -> Error {
    let id = match failure.code() {
        "APP.INVALID_NAME" => "config.name",
        _ => "config.invalid",
    };
    error(id, failure)
}
fn invalid_setting(resolved: &Resolved, path: &str, failure: impl ToString) -> Error {
    let sources = resolved
        .rows()
        .into_iter()
        .filter(|row| {
            let key = row.path.join(".");
            key == path || key.starts_with(&format!("{path}."))
        })
        .map(|row| row.source.name)
        .collect::<Vec<_>>()
        .join(", ");
    error(
        "config.invalid",
        format!("{path} from {sources}: {}", failure.to_string()),
    )
}
impl ConfigSources<'_> {
    pub fn load(&self) -> Result<Configuration, Error> {
        let environment = self.environment.0.expose();
        let (local, global, project) =
            (self.local.as_deref(), self.global.as_deref(), self.project);
        if let Some((name, _)) = environment
            .iter()
            .find(|(name, _)| name.to_string_lossy().starts_with("DECIDR_MODELS"))
        {
            return Err(error(
                "config.invalid",
                format!("{name:?}: models is set in files only"),
            ));
        }
        let keys = config_keys();
        let global = global
            .map(Path::to_path_buf)
            .or_else(|| {
                environment
                    .iter()
                    .find(|(name, value)| name == "SC_AGENT_HOME" && !value.is_empty())
                    .map(|(_, value)| PathBuf::from(value))
            })
            .or_else(|| std::env::home_dir().map(|home| home.join(".sc-agent")))
            .ok_or_else(|| error("config.home", "Home directory unavailable"))?;
        let context = StandardContext::new(
            AppName::new("sc-agent").map_err(config_error)?,
            vec![],
            vec![],
        );
        let mut layers = StandardLayers::new(context).defaults(json!({"workers":8,"model":"jev"}));
        let mut files = Vec::new();
        for (slot, folder) in [(Slot::User, Some(global.as_path())), (Slot::Local, local)] {
            if let Some(folder) = folder {
                for name in ["sc-agent.json", "decidr.json", "decidr.local.json"] {
                    let path = folder.join(name);
                    files.push(json!({"path":path,"exists":path.exists()}));
                    layers = layers.insert(
                        slot,
                        FileSource::new(path)
                            .optional()
                            .accepts(keys.iter().map(|key| key.split('.').collect::<Vec<_>>())),
                    );
                }
            }
        }
        let flags = project
            .map(|project| json!({"project":project}))
            .unwrap_or(json!({}));
        let resolved = layers
            .insert(Slot::Env, self.environment.clone())
            .flags(flags)
            .resolve()
            .map_err(config_error)?;
        let config = Settings {
            project: setting(&resolved, "project", Value::Null)?,
            workers: setting(&resolved, "workers", json!(8))?,
            model: setting(&resolved, "model", Value::Null)?,
            models: setting::<BTreeMap<String, ModelEntry>>(&resolved, "models", json!({}))?
                .into_iter()
                .map(|(name, entry)| {
                    Ok((
                        name,
                        ConfiguredRow {
                            provider: providers::named(&entry.provider)?,
                            id: entry.id,
                            released: entry.released,
                        },
                    ))
                })
                .collect::<Result<_, Error>>()?,
            local: providers::PROVIDERS
                .iter()
                .filter_map(|p| match p.base {
                    Base::Local(name) => Some(name),
                    _ => None,
                })
                .filter(|name| resolved.value.get(*name).is_some())
                .map(|name| {
                    for field in ["host", "port"] {
                        if resolved
                            .value
                            .pointer(&format!("/{name}/{field}"))
                            .is_none()
                        {
                            return Err(invalid_setting(
                                &resolved,
                                name,
                                missing_setting(&format!("{name}.{field}")),
                            ));
                        }
                    }
                    Ok((
                        name,
                        Endpoint {
                            host: setting(&resolved, &format!("{name}.host"), Value::Null)?,
                            port: setting(&resolved, &format!("{name}.port"), Value::Null)?,
                        },
                    ))
                })
                .collect::<Result<_, Error>>()?,
        };
        if config.workers == 0 {
            return Err(invalid_setting(&resolved, "workers", "must be positive"));
        }
        let provenance = resolved.rows();
        let rows = keys
            .iter()
            .map(|key| {
                let row = provenance.iter().find(|row| row.path.join(".") == *key);
                let value = row.map_or_else(
                    || "unset".into(),
                    |row| {
                        row.value
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| row.value.to_string())
                    },
                );
                let source = row.map_or_else(
                    || "default".into(),
                    |row| match row.source.kind {
                        SourceKind::Default => "default".into(),
                        SourceKind::Flag => "--project".into(),
                        _ => row.source.name.clone(),
                    },
                );
                json!({"path":key,"value":value,"source":source})
            })
            .collect();
        let mut rows: Vec<Value> = rows;
        rows.extend(provenance.iter().filter(|row|row.path.first().is_some_and(|s|s=="models") && row.path.len()>1).map(|row|json!({"path":row.path.join("."),"value":row.value,"source":row.source.name})));
        files.reverse();
        Ok(Configuration {
            config,
            rows,
            files,
        })
    }
}
pub fn slug(value: &str) -> Result<(), Error> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || !value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
    {
        return Err(error(
            "log.invalid_slug",
            "project and agent_role must be lowercase slugs",
        ));
    }
    Ok(())
}
pub fn default_logger(name: &str) -> Result<Logger, Error> {
    logger_at(name, Environment::process().log_root(name)?)
}
fn logger_at(name: &str, root: PathBuf) -> Result<Logger, Error> {
    slug(name)?;
    let mut config = LoggerConfig::default_for(
        ServiceName::new(name).map_err(|e| error("config.logger", e))?,
        root,
    );
    config.enable_console_sink = false;
    Logger::new(config).map_err(|e| error("config.logger", e))
}
#[derive(Debug)]
pub enum KeyringError {
    Read(String),
    Timeout,
}
fn keyring_result(result: Result<String, keyring::Error>) -> Result<String, String> {
    match result {
        Ok(value) => Ok(value),
        Err(keyring::Error::NoEntry) => Ok(String::new()),
        Err(error) => Err(error.to_string()),
    }
}
fn keyring_wait(
    reader: impl FnOnce() -> Result<String, String> + Send + 'static,
    timeout: Duration,
) -> Result<String, KeyringError> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let _ = sender.send(reader());
    });
    match receiver.recv_timeout(timeout) {
        Ok(value) => value.map_err(KeyringError::Read),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(KeyringError::Timeout),
        Err(error) => Err(KeyringError::Read(error.to_string())),
    }
}
pub fn os_keyring(variable: &str) -> Result<String, KeyringError> {
    let variable = variable.to_owned();
    let account = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .map_err(|e| KeyringError::Read(e.to_string()))?;
    keyring_wait(
        move || {
            let result =
                keyring::Entry::new(&variable, &account).and_then(|entry| entry.get_password());
            keyring_result(result)
        },
        Duration::from_secs(15),
    )
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Bounds {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Answer {
    Choice {
        choice: String,
        confidence: f64,
        probabilities: BTreeMap<String, f64>,
    },
    Score {
        score: f64,
        confidence: f64,
        legend: BTreeMap<String, String>,
        probabilities: BTreeMap<String, f64>,
    },
    Noul {
        noul: f64,
    },
    Box {
        r#box: Option<Bounds>,
        confidence: Option<f64>,
    },
}
pub fn schema(request: &Value, response: &Value) -> Result<(), Error> {
    let questions = request
        .get("questions")
        .and_then(Value::as_object)
        .ok_or_else(|| error("decision.request_parse", "questions must be an object"))?;
    let answers = response
        .get("answers")
        .and_then(Value::as_object)
        .ok_or_else(|| error("decision.schema_mismatch", "answers must be an object"))?;
    if questions.len() != answers.len() {
        return Err(error(
            "decision.schema_mismatch",
            "answer ids do not match question ids",
        ));
    }
    for (id, question) in questions {
        let answer = answers
            .get(id)
            .ok_or_else(|| error("decision.schema_mismatch", format!("missing answer {id}")))?;
        let _: Answer = serde_json::from_value(answer.clone())
            .map_err(|e| error("decision.schema_mismatch", format!("{id}: {e}")))?;
        if answer.get("type") != question.get("type") {
            return Err(error(
                "decision.schema_mismatch",
                format!("{id}: wrong question type"),
            ));
        }
    }
    Ok(())
}
pub fn digest(request: &Value, question: &str) -> Result<String, Error> {
    let mut input = request
        .as_object()
        .cloned()
        .ok_or_else(|| error("decision.request_parse", "request must be an object"))?;
    let selected = input
        .remove("questions")
        .and_then(|v| v.get(question).cloned())
        .ok_or_else(|| error("decision.request_parse", "question id missing"))?;
    input.insert("questions".into(), json!({question:selected}));
    let text = serde_json::to_vec(&input).map_err(|e| error("decision.encode", e))?;
    sc_sha::calculate_hash(sc_sha::HashInput::TextFileBytes {
        utf8_file_bytes: &text,
    })
    .map(|h| h.template().to_string())
    .map_err(|e| error("decision.encode", e))
}

pub type Wait = dyn Fn(Duration, &Cancel) + Send + Sync;
#[derive(Clone)]
pub struct Seams {
    pub base_url: Option<String>,
    pub wait: Arc<Wait>,
    pub keyring: Arc<providers::KeyringLookup>,
    pub environment: Option<Environment>,
    pub repository: Option<Repository>,
}
impl Default for Seams {
    fn default() -> Self {
        Self {
            base_url: None,
            wait: Arc::new(|delay, cancel| cancel.wait(delay)),
            keyring: Arc::new(os_keyring),
            environment: None,
            repository: None,
        }
    }
}
pub struct Context {
    pub settings: Settings,
    pub keys: KeySources,
    pub repository: Repository,
    pub log_root: PathBuf,
}
#[derive(Clone, Default)]
pub struct RunConfig {
    pub tool: String,
    pub tool_version: String,
    pub model: Option<String>,
    pub agent_role: Option<String>,
    pub project: Option<String>,
    pub config_dir: Option<PathBuf>,
    pub workers: Option<i64>,
    pub timeout_secs: Option<f64>,
    pub logger: Option<Arc<Logger>>,
    pub log_name: String,
    pub cancel: Option<Arc<Cancel>>,
    pub seams: Seams,
}
impl RunConfig {
    pub fn new(tool: &str, tool_version: &str) -> Self {
        Self {
            tool: tool.into(),
            tool_version: tool_version.into(),
            log_name: "decidr".into(),
            ..Self::default()
        }
    }
}
pub struct Run {
    ctx: Context,
    model: ResolvedModel,
    agent_role: Option<String>,
    timeout: Duration,
    workers: usize,
    logger: Arc<Logger>,
    owns_logger: bool,
    cancel: Arc<Cancel>,
    seams: Seams,
    closed: AtomicBool,
}
#[derive(Default)]
pub struct Call<'a> {
    pub agent_role: Option<&'a str>,
}
impl Run {
    pub fn new(config: RunConfig) -> Result<Self, Error> {
        let environment = config
            .seams
            .environment
            .clone()
            .unwrap_or_else(Environment::process);
        let repository = match &config.seams.repository {
            Some(r) => r.clone(),
            None => Repository::discover()?,
        };
        let settings = ConfigSources::new(
            &repository,
            config.config_dir.as_deref(),
            &environment,
            config.project.as_deref(),
        )
        .load()?
        .config;
        let ctx = Context {
            log_root: environment.log_root(&config.log_name)?,
            keys: KeySources::new(environment, &repository.root, config.seams.keyring.clone())?,
            settings,
            repository,
        };
        let workers = config
            .workers
            .map(workers)
            .transpose()?
            .unwrap_or(ctx.settings.workers);
        let timeout = Duration::try_from_secs_f64(config.timeout_secs.unwrap_or(120.0))
            .map_err(|e| error("decidr.invalid_request_timeout", e))?;
        let model =
            providers::resolve(config.model.as_deref().unwrap_or(&ctx.settings.model), &ctx)?;
        if let Base::Local(name) = model.provider.base {
            ctx.settings
                .local
                .get(name)
                .ok_or_else(|| missing_setting(&format!("{name}.host")))?;
        }
        let owns_logger = config.logger.is_none();
        let logger = match config.logger {
            Some(l) => l,
            None => Arc::new(logger_at(&config.log_name, ctx.log_root.clone())?),
        };
        Ok(Self {
            ctx,
            model,
            agent_role: config.agent_role,
            timeout,
            workers,
            logger,
            owns_logger,
            cancel: config.cancel.unwrap_or_default(),
            seams: config.seams,
            closed: AtomicBool::new(false),
        })
    }
    pub fn model(&self) -> &ResolvedModel {
        &self.model
    }
    pub fn cancel(&self) {
        self.cancel.cancel();
    }
    pub fn close(&self) -> Result<(), Error> {
        if self.closed.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        self.logger.flush().map_err(|e| error("log.flush", e))?;
        if self.owns_logger {
            self.logger
                .shutdown()
                .map_err(|e| error("log.shutdown", e))?;
        }
        Ok(())
    }
    fn sender<'a>(
        &'a self,
        call: &'a Call<'a>,
        wait: &'a (dyn Fn(Duration) + Sync),
    ) -> Result<Sender<'a>, Error> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(error("decision.run_closed", "Run is closed"));
        }
        Ok(Sender {
            ctx: &self.ctx,
            model: &self.model,
            agent_role: call.agent_role.or(self.agent_role.as_deref()).unwrap_or(""),
            timeout: self.timeout,
            workers: self.workers,
            logger: &self.logger,
            cancel: &self.cancel,
            base_url: self.seams.base_url.as_deref(),
            wait,
        })
    }
    pub fn send(&self, call: &Call<'_>, request: &Value) -> Result<Value, Error> {
        self.sender(call, &|delay| (self.seams.wait)(delay, &self.cancel))?
            .send_one(request)
    }
    pub fn send_batch(
        &self,
        call: &Call<'_>,
        requests: &[Value],
        on_result: &(dyn Fn(usize, &Value) + Sync),
    ) -> Result<Value, Error> {
        self.sender(call, &|delay| (self.seams.wait)(delay, &self.cancel))?
            .send_batch(requests, on_result)
    }
}
struct Sender<'a> {
    ctx: &'a Context,
    model: &'a ResolvedModel,
    agent_role: &'a str,
    timeout: Duration,
    workers: usize,
    logger: &'a Logger,
    cancel: &'a Cancel,
    base_url: Option<&'a str>,
    wait: &'a (dyn Fn(Duration) + Sync),
}
pub fn event(
    level: sc_observability::Level,
    target: &str,
    action: &str,
    fields: Value,
) -> Result<sc_observability::LogEvent, Error> {
    serde_json::from_value(json!({"version":"v1","timestamp":sc_observability::Timestamp::now_utc(),"level":level,"service":target,"target":target,"action":action,"identity":sc_observability::ProcessIdentity::default(),"fields":fields})).map_err(|e|error("log.invalid_event",e))
}
pub fn log(logger: &Logger, action: &str, fields: Value) -> Result<(), Error> {
    let mut event = event(
        if action == "error" {
            sc_observability::Level::Error
        } else {
            sc_observability::Level::Info
        },
        "sc-agent",
        action,
        fields,
    )?;
    event.service = logger.service_name().clone();
    logger.log(event).map_err(|e| error("log.write", e))
}

fn answer_schema(question: &Value) -> Value {
    let kind = question.get("type").and_then(Value::as_str).unwrap_or("");
    let names = match question.get("criteria") {
        Some(Value::Object(v)) => v.keys().cloned().collect::<Vec<_>>(),
        Some(Value::Array(v)) => (0..v.len()).map(|i| i.to_string()).collect(),
        _ => vec![],
    };
    let object = |value_type| json!({"type":"object","properties":names.iter().map(|n|(n,json!({"type":value_type}))).collect::<BTreeMap<_,_>>(),"required":names,"additionalProperties":false});
    let fields = match kind {
        "choice" => {
            json!({"type":{"type":"string","enum":[kind]},"choice":{"type":"string","enum":names},"confidence":{"type":"number"},"probabilities":object("number")})
        }
        "score" => {
            json!({"type":{"type":"string","enum":[kind]},"score":{"type":"number"},"confidence":{"type":"number"},"legend":object("string"),"probabilities":object("number")})
        }
        "box" => {
            json!({"type":{"type":"string","enum":[kind]},"box":{"type":["object","null"],"properties":(["top","bottom","left","right"].into_iter().map(|n|(n,json!({"type":"number"}))).collect::<BTreeMap<_,_>>()),"required":["top","bottom","left","right"],"additionalProperties":false},"confidence":{"type":["number","null"]}})
        }
        _ => json!({"type":{"type":"string","enum":[kind]},"noul":{"type":"number"}}),
    };
    let required = fields
        .as_object()
        .map(|v| v.keys().collect::<Vec<_>>())
        .unwrap_or_default();
    json!({"type":"object","properties":fields,"required":required,"additionalProperties":false})
}
fn wire(request: &Value, model: &str, wire: Wire) -> Value {
    if wire == Wire::SystemOne {
        let mut body = request.clone();
        if let Some(body) = body.as_object_mut() {
            body.insert("model".into(), json!(model));
        }
        return body;
    }
    let properties = request
        .get("questions")
        .and_then(Value::as_object)
        .map(|questions| {
            questions
                .iter()
                .map(|(id, question)| (id.clone(), answer_schema(question)))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let required = properties.keys().cloned().collect::<Vec<_>>();
    let schema = json!({
        "type":"object", "required":["answers"], "additionalProperties":false,
        "properties":{"answers":{"type":"object","properties":properties,"required":required,"additionalProperties":false}}
    });
    let messages = json!([{"role":"user","content":format!("Answer every question in this decision request using its required schema, including probabilities and confidence: {request}")}]);
    if wire == Wire::Messages {
        json!({"model":model,"max_tokens":8192,"messages":messages,"output_config":{"format":{"type":"json_schema","schema":schema}}})
    } else {
        json!({"model":model,"messages":messages,"response_format":{"type":"json_schema","json_schema":{"name":"decision","strict":true,"schema":schema}}})
    }
}
fn response(request: &Value, raw: Value, wire: Wire) -> Result<Value, Error> {
    let answer = match wire {
        Wire::Messages => raw
            .get("content")
            .and_then(Value::as_array)
            .and_then(|items| {
                items
                    .iter()
                    .find_map(|v| v.get("text").and_then(Value::as_str))
            }),
        Wire::ChatCompletions => raw
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str),
        _ => {
            schema(request, &raw)?;
            return Ok(raw);
        }
    }
    .ok_or_else(|| {
        error(
            "decision.response_parse",
            format!("provider reply has no text: {raw}"),
        )
    })?;
    let parsed: Value = serde_json::from_str(answer)
        .map_err(|e| error("decision.response_parse", format!("{e}: {answer}")))?;
    schema(request, &parsed)?;
    Ok(parsed)
}
static HOLDOFFS: Mutex<BTreeMap<String, Arc<Mutex<()>>>> = Mutex::new(BTreeMap::new());
static SESSION_APPEND: Mutex<()> = Mutex::new(());

pub fn host_holdoff(host: &str) -> Result<Arc<Mutex<()>>, Error> {
    Ok(HOLDOFFS
        .lock()
        .map_err(|e| error("http.network", e))?
        .entry(host.to_owned())
        .or_default()
        .clone())
}

pub fn validate_request_size(request: &Value) -> Result<(), Error> {
    if serde_json::to_vec(request)
        .map_err(|e| error("decision.encode", e))?
        .len()
        > 8 * 1024 * 1024
    {
        return Err(error(
            "decision.request_too_large",
            "request exceeds the 8 MiB serialized input limit",
        ));
    }
    Ok(())
}
pub fn workers(value: i64) -> Result<usize, Error> {
    usize::try_from(value)
        .ok()
        .filter(|v| *v > 0)
        .ok_or_else(|| error("decidr.invalid_workers", "workers must be positive"))
}

impl Sender<'_> {
    fn send_one(&self, request: &Value) -> Result<Value, Error> {
        let logger = self.logger;
        let cancel = self.cancel;
        let wait = self.wait;
        let session_id = ulid::Ulid::new().to_string();
        let digests = request
            .get("questions")
            .and_then(Value::as_object)
            .map(|questions| {
                questions
                    .keys()
                    .filter_map(|id| digest(request, id).ok().map(|hash| (id.clone(), hash)))
                    .collect::<BTreeMap<_, _>>()
            });
        let mut sends = 0;
        let mut outcome = (|| {
            validate_request_size(request)?;
            cancel.check()?;
            let project = self.ctx.settings.project_required()?;
            slug(project)?;
            slug(self.agent_role)?;
            let provider = self.model.provider;
            let model = &self.model.id;
            let base = match provider.base {
                Base::Url(url) => url.to_owned(),
                Base::Local(name) => {
                    let endpoint = self
                        .ctx
                        .settings
                        .local
                        .get(name)
                        .ok_or_else(|| missing_setting(&format!("{name}.host")))?;
                    format!("http://{}:{}/v1", endpoint.host, endpoint.port)
                }
            };
            let url = format!(
                "{}/{}",
                self.base_url.unwrap_or(&base).trim_end_matches('/'),
                provider.wire.path()
            );
            let uri = url
                .parse::<ureq::http::Uri>()
                .map_err(|e| error("http.network", e))?;
            let host = uri
                .host()
                .ok_or_else(|| error("http.network", "endpoint has no host"))?
                .to_ascii_lowercase();
            let loopback = host == "localhost"
                || (host.trim_matches(['[', ']']).parse::<std::net::IpAddr>())
                    .is_ok_and(|ip| ip.is_loopback());
            if provider.key.is_some() && uri.scheme_str() != Some("https") && !loopback {
                return Err(error(
                    "http.key_on_plain_http",
                    format!(
                        "{} is never sent to the plain-http host {host}",
                        provider.key.unwrap_or("")
                    ),
                ));
            }
            let pause = host_holdoff(&host)?;
            let key = self
                .model
                .credential
                .as_ref()
                .map(|c| c.value.expose().as_str())
                .unwrap_or("");
            let body = wire(request, model, provider.wire);
            let agent = ureq::Agent::config_builder()
                .http_status_as_error(false)
                .timeout_global(Some(self.timeout))
                .build()
                .new_agent();
            for attempt in 1..=5 {
                cancel.check()?;
                // A busy reply holds off senders to this host through its injected wait.
                drop(pause.lock().map_err(|e| error("http.network", e))?);
                cancel.check()?;
                let mut post = agent.post(&url).header("Content-Type", "application/json");
                if provider.key.is_some() {
                    post = provider.wire.auth(post, key);
                }
                sends = attempt;
                // A send in flight cannot be aborted, so on cancel it is abandoned and the request ends now.
                let sending = body.clone();
                let sent = cancel.run(move || {
                    post.send_json(&sending).map(|mut reply| {
                        let delay = reply
                            .headers()
                            .get("Retry-After")
                            .and_then(|h| h.to_str().ok())
                            .and_then(|h| h.parse::<f64>().ok())
                            .and_then(|s| Duration::try_from_secs_f64(s).ok());
                        (
                            reply.status().as_u16(),
                            delay,
                            reply.body_mut().read_to_string(),
                        )
                    })
                })?;
                let (status, delay, text) = match sent {
                    Ok((status, delay, text)) => {
                        let text = text.map_err(|e| error("decision.response_parse", e))?;
                        (status, delay, providers::redact(&text, key).into_owned())
                    }
                    Err(e) => {
                        if attempt < 5 {
                            wait(Duration::from_secs(2u64.pow(attempt - 1)));
                            continue;
                        }
                        return Err(
                            error("http.network", providers::redact(&e.to_string(), key))
                                .attempts(attempt),
                        );
                    }
                };
                if (200..300).contains(&status) {
                    let decoded = serde_json::from_str(&text)
                        .map_err(|e| error("decision.response_parse", format!("{e}: {text}")))
                        .and_then(|raw: Value| {
                            if matches!(provider.base, Base::Local(_))
                                && raw
                                    .pointer(provider.wire.reply().model)
                                    .and_then(Value::as_str)
                                    .is_some_and(|id| id != model)
                            {
                                return Err(error(
                                    "decision.model_mismatch",
                                    format!(
                                        "requested {model}, received {}",
                                        raw.pointer(provider.wire.reply().model)
                                            .unwrap_or(&Value::Null)
                                    ),
                                ));
                            }
                            response(request, raw, provider.wire)
                        });
                    return decoded;
                }
                let low = text.to_lowercase();
                let quota = status == 402
                    || [
                        "insufficient_quota",
                        "quota_exhausted",
                        "credit balance",
                        "spend limit",
                    ]
                    .iter()
                    .any(|needle| low.contains(needle));
                let busy = [429, 502, 529].contains(&status) && !quota;
                if busy && attempt < 5 {
                    let _hold = pause.lock().map_err(|e| error("http.network", e))?;
                    wait(delay.unwrap_or_else(|| Duration::from_secs(2u64.pow(attempt - 1))));
                    continue;
                }
                return Err(error(
                    if quota {
                        "http.quota_exhausted"
                    } else if busy {
                        "http.provider_busy"
                    } else if status == 401 {
                        "http.auth"
                    } else {
                        "decision.api_status"
                    },
                    text,
                )
                .attempts(attempt));
            }
            Err(error("http.network", "send count exhausted"))
        })();
        if cancel.is_cancelled() {
            outcome = Err(error("decision.cancelled", "request cancelled"));
        }
        if let Err(Error::Failure {
            session_id: id,
            attempts,
            ..
        }) = &mut outcome
        {
            *id = Some(session_id.clone());
            if cancel.is_cancelled() {
                *attempts = Some(sends);
            }
        }
        if let Ok(value) = &mut outcome
            && let Some(fields) = value.as_object_mut()
        {
            fields.insert("attempts".into(), json!(sends));
            fields.insert("session_id".into(), json!(session_id));
            fields.insert("question_digests".into(), json!(digests));
        }
        if let Ok(project) = self.ctx.settings.project_required()
            && slug(project).is_ok()
            && slug(self.agent_role).is_ok()
        {
            let row = json!({"session_id":session_id,"repo":self.ctx.repository.identity,"agent_role":self.agent_role,"project":project,"request":request,"question_digests":digests,"result":match &outcome {Ok(v)=>json!({"ok":true,"value":v}),Err(e)=>json!({"ok":false,"error":e})}});
            let path = self
                .ctx
                .log_root
                .join("sessions")
                .join(&self.ctx.repository.identity)
                .join(self.agent_role)
                .join(project);
            let _append = SESSION_APPEND
                .lock()
                .map_err(|e| error("config.session", e))?;
            let written = (|| -> std::io::Result<()> {
                use std::io::Write;
                std::fs::create_dir_all(&path)?;
                writeln!(
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(path.join("sessions.jsonl"))?,
                    "{row}"
                )
            })();
            if let Err(e) = written {
                log(logger, "error", json!(error("log.session_write", e)))?;
            }
        }
        if let Ok(value) = &outcome {
            log(logger, "decision.result", json!({"response":value}))?;
        }
        logger.flush().map_err(|e| error("log.flush", e))?;
        outcome
    }
    fn send_batch(
        &self,
        requests: &[Value],
        on_result: &(dyn Fn(usize, &Value) + Sync),
    ) -> Result<Value, Error> {
        let logger = self.logger;
        requests.iter().try_for_each(validate_request_size)?;
        let next = std::sync::atomic::AtomicUsize::new(0);
        let (sender, receiver) = std::sync::mpsc::channel();
        Ok(std::thread::scope(|scope| {
            for _ in 0..self.workers.min(requests.len()) {
                let sender = sender.clone();
                let next = &next;
                scope.spawn(move || {
                    loop {
                        let index = next.fetch_add(1, Ordering::SeqCst);
                        let Some(request) = requests.get(index) else {
                            break;
                        };
                        let value = report(logger, self.send_one(request));
                        on_result(index, &value);
                        let _ = sender.send((index, value));
                    }
                });
            }
            drop(sender);
            let mut results = receiver.into_iter().collect::<Vec<_>>();
            results.sort_by_key(|(index, _)| *index);
            json!(
                results
                    .into_iter()
                    .map(|(index, value)| json!({"index":index,"result":value}))
                    .collect::<Vec<_>>()
            )
        }))
    }
}
impl Repository {
    pub fn discover() -> Result<Self, Error> {
        let cwd = std::env::current_dir().map_err(|e| error("config.repo", e))?;
        let command = |name: &str, args: &[&str]| {
            std::process::Command::new(name)
                .args(args)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        };
        let root = command("git", &["rev-parse", "--show-toplevel"])
            .map(PathBuf::from)
            .unwrap_or(cwd);
        let origin = command("git", &["remote", "get-url", "origin"]);
        let host = command("hostname", &[])
            .filter(|s| !s.is_empty())
            .ok_or_else(|| error("config.repo", "host name unavailable"))?;
        let name = repository_identity(origin.as_deref(), &host, &root)?;
        Ok(Self {
            root,
            identity: name,
        })
    }
}
pub fn repository_identity(origin: Option<&str>, host: &str, root: &Path) -> Result<String, Error> {
    let remote = origin
        .map(|s| s.trim_end_matches('/').trim_end_matches(".git"))
        .map(|s| {
            s.split_once("://")
                .map_or_else(|| s.replacen(':', "/", 1), |(_, s)| s.to_owned())
        })
        .and_then(|s| {
            let s = s.split_once('@').map(|(_, s)| s).unwrap_or(&s);
            let (host, path) = s.split_once('/')?;
            let host = host.split_once(':').map(|(host, _)| host).unwrap_or(host);
            (!host.is_empty()).then(|| format!("{host}/{path}"))
        });
    let name = remote.unwrap_or_else(|| {
        format!(
            "local/{host}/{}",
            root.file_name().unwrap_or_default().to_string_lossy()
        )
    });
    if name
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(error("config.repo", "invalid repository identity"));
    }
    Ok(name)
}

pub fn report(logger: &Logger, outcome: Result<Value, Error>) -> Value {
    if let Err(error) = &outcome
        && let Err(failure) = log(logger, "error", json!(error))
    {
        return result(Err(failure));
    }
    result(outcome)
}

// compat: deleted in j-2
pub const PROVIDERS: &[&providers::Provider] = providers::PROVIDERS;
pub struct Options {
    pub environment: Vec<(OsString, OsString)>,
    pub settings: Settings,
    pub model: String,
    pub provider: Option<String>,
    pub agent_role: String,
    pub timeout: Duration,
    pub base_url: Option<String>,
    pub root: PathBuf,
    pub log_root: PathBuf,
    pub repo: String,
}
pub fn repository() -> Result<(PathBuf, String), Error> {
    Repository::discover().map(|r| (r.root, r.identity))
}
pub fn log_root(name: &str) -> Result<PathBuf, Error> {
    Environment::process().log_root(name)
}
pub fn config(
    local: Option<&Path>,
    global: Option<&Path>,
    environment: &[(OsString, OsString)],
    project: Option<&str>,
) -> Result<Configuration, Error> {
    ConfigSources {
        local: local.map(Into::into),
        global: global.map(Into::into),
        environment: &Environment(Secret::new(environment.to_vec())),
        project,
    }
    .load()
}
pub fn model(
    name: &str,
    provider: Option<&str>,
) -> Result<(&'static providers::Provider, String), Error> {
    model_with(name, provider, &BTreeMap::new())
}
pub fn model_with(
    name: &str,
    provider: Option<&str>,
    configured: &BTreeMap<String, ConfiguredRow>,
) -> Result<(&'static providers::Provider, String), Error> {
    let settings = Settings {
        project: None,
        workers: 1,
        model: String::new(),
        models: configured.clone(),
        local: BTreeMap::new(),
    };
    if let Some(name) = provider {
        providers::named(name)?;
    }
    providers::candidates(name, &settings)
        .into_iter()
        .find(|r| provider.is_none_or(|p| p == r.provider.name))
        .map(|r| (r.provider, r.id.into()))
        .ok_or_else(|| error("types.unknown_short_name", name))
}
pub fn doctor(config_dir: Option<&Path>, project: Option<&str>) -> Result<Value, Error> {
    let repository = Repository::discover()?;
    let environment = Environment::process();
    let sources = ConfigSources::new(&repository, config_dir, &environment, project);
    let loaded = sources.load()?;
    let keys = KeySources::new(environment.clone(), &repository.root, Arc::new(os_keyring))?;
    Ok(
        json!({"settings":loaded.rows,"files":loaded.files,"log_root":environment.log_root("decidr")?,"keys":providers::doctor_key_rows(&keys)}),
    )
}
struct Legacy<'a> {
    options: &'a Options,
    logger: &'a Logger,
    cancel: &'a Cancel,
    wait: &'a (dyn Fn(Duration) + Sync),
    keyring: &'a (dyn Fn(&str) -> Result<String, KeyringError> + Sync),
}
impl Legacy<'_> {
    fn invoke(&self, f: impl FnOnce(Sender<'_>) -> Result<Value, Error>) -> Result<Value, Error> {
        let o = self.options;
        let mut ctx = Context {
            settings: o.settings.clone(),
            keys: KeySources::new(
                Environment(Secret::new(o.environment.clone())),
                &o.root,
                Arc::new(os_keyring),
            )?,
            repository: Repository {
                root: o.root.clone(),
                identity: o.repo.clone(),
            },
            log_root: o.log_root.clone(),
        };
        if o.provider.is_some() {
            let (provider, id) = model_with(&o.model, o.provider.as_deref(), &ctx.settings.models)?;
            ctx.settings.models.insert(
                o.model.clone(),
                ConfiguredRow {
                    provider,
                    id,
                    released: None,
                },
            );
        }
        let model = providers::resolve_using(&o.model, &ctx, &|key| {
            ctx.keys.find_using(key, self.keyring)
        })?;
        f(Sender {
            ctx: &ctx,
            model: &model,
            agent_role: &o.agent_role,
            timeout: o.timeout,
            workers: o.settings.workers,
            logger: self.logger,
            cancel: self.cancel,
            wait: self.wait,
            base_url: o.base_url.as_deref(),
        })
    }
}
pub fn send(
    request: &Value,
    options: &Options,
    logger: &Logger,
    cancel: &Cancel,
    wait: &(dyn Fn(Duration) + Sync),
    keyring: &(dyn Fn(&str) -> Result<String, KeyringError> + Sync),
) -> Result<Value, Error> {
    Legacy {
        options,
        logger,
        cancel,
        wait,
        keyring,
    }
    .invoke(|call| call.send_one(request))
}
pub fn batch(
    requests: &[Value],
    options: &Options,
    logger: &Logger,
    cancel: &Cancel,
    wait: &(dyn Fn(Duration) + Sync),
    keyring: &(dyn Fn(&str) -> Result<String, KeyringError> + Sync),
    on_result: &(dyn Fn(usize, &Value) + Sync),
) -> Result<Value, Error> {
    Legacy {
        options,
        logger,
        cancel,
        wait,
        keyring,
    }
    .invoke(|call| call.send_batch(requests, on_result))
}

#[cfg(test)]
extern crate self as sc_agent;
#[cfg(test)]
#[path = "../tests/support/mod.rs"]
pub mod test_support;
#[cfg(test)]
mod run_tests {
    use super::test_support::*;
    use super::*;
    #[test]
    fn local_and_remote_timeout_defaults_and_overrides_are_resolved_once() -> Test {
        let root = Temp::new()?;
        let stub = Stub::new(vec![])?;
        std::fs::write(root.0.join("sc-agent.json"),json!({"laya":{"host":"unused.invalid","port":1},"decider_4b":{"host":"unused.invalid","port":1}}).to_string())?;
        for selector in ["laya", "decider", "jev"] {
            for timeout in [None, Some(5.0)] {
                let mut cfg = run_config(&root, &stub)?;
                cfg.model = Some(selector.into());
                cfg.timeout_secs = timeout;
                let run = Run::new(cfg)?;
                assert_eq!(
                    run.timeout,
                    Duration::from_secs(if timeout.is_some() { 5 } else { 120 })
                );
                run.close()?;
                assert!(run.logger.flush().is_err());
            }
        }
        assert!(stub.finish()?.is_empty());
        Ok(())
    }
    #[test]
    fn keyring_timeout_is_driven_by_an_unreleased_reader() -> Test {
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let value = keyring_wait(
            move || {
                let _ = wait.recv_timeout(Duration::from_secs(5));
                Ok("unused".into())
            },
            Duration::ZERO,
        );
        let _ = release.send(());
        assert!(matches!(value, Err(KeyringError::Timeout)));
        assert!(matches!(
            keyring_wait(|| Err("controlled refusal".into()), Duration::from_secs(5)),
            Err(KeyringError::Read(_))
        ));
        assert!(keyring_result(Err(keyring::Error::NoEntry))?.is_empty());
        Ok(())
    }
}
