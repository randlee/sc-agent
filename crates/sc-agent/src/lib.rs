use sc_config::{
    AppName, ConfigError, EnvSource, FileSource, Resolved, Slot, SourceKind, StandardContext,
    StandardLayers,
};
use sc_observability::{LoggerConfig, ServiceName, v2::Logger};
use serde::{Deserialize, Serialize, de::Error as DeError};
use serde_json::{Value, json};
use std::{
    cell::Cell,
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex, MutexGuard, TryLockError,
        atomic::Ordering,
        mpsc::{Receiver, RecvTimeoutError},
    },
    time::{Duration, Instant},
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
    pub fn check(&self) -> Result<(), Error> {
        match self.is_cancelled() {
            true => Err(error("decision.cancelled", "request cancelled")),
            false => Ok(()),
        }
    }
    pub fn recv<T>(&self, receiver: &Receiver<T>, id: &'static str) -> Result<T, Error> {
        loop {
            self.check()?;
            match receiver.recv_timeout(Duration::from_millis(20)) {
                Err(RecvTimeoutError::Timeout) => {}
                other => return other.map_err(|e| error(id, e)),
            }
        }
    }
    pub fn lock<'a, T>(&self, mutex: &'a Mutex<T>) -> Result<MutexGuard<'a, T>, Error> {
        loop {
            self.check()?;
            match mutex.try_lock() {
                Ok(guard) => return Ok(guard),
                Err(TryLockError::WouldBlock) => {
                    std::thread::park_timeout(Duration::from_millis(20))
                }
                Err(TryLockError::Poisoned(e)) => return Err(error("http.network", e)),
            }
        }
    }
    pub fn wait(&self, delay: Duration) -> Result<(), Error> {
        if let Ok(state) = self.state.lock() {
            let _ = self
                .changed
                .wait_timeout_while(state, delay, |state| !*state);
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, thiserror::Error)]
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
/// Keep an earlier error and append a later failure's detail.
pub fn after_failure(
    mut value: Value,
    completion: Result<(), impl ToString>,
    id: &'static str,
) -> Value {
    let Err(failure) = completion else {
        return value;
    };
    if let Some(detail) = value.get_mut("error").and_then(Value::as_object_mut) {
        let message = detail.get("message").and_then(Value::as_str).unwrap_or("");
        detail.insert(
            "message".into(),
            json!(format!("{message}; {id}: {}", failure.to_string())),
        );
        value
    } else {
        result(Err(error(id, failure)))
    }
}
pub fn error(id: &'static str, message: impl ToString) -> Error {
    let fix_input = "Fix the named input or provider response.";
    let retry = "Retry later.";
    let (category, action) = match id {
        "secrets.keyring_timeout" => (
            "auth",
            "Approve keychain access, or set the environment variable or .env key.",
        ),
        "secrets.keyring_read" => (
            "auth",
            "Restore platform keyring access, or set the key in the environment or repository .env.",
        ),
        "secrets.key_missing" => (
            "auth",
            "Set the key in the environment, repository .env, or platform keyring.",
        ),
        "secrets.key_malformed" | "http.auth" => ("auth", "Check the provider API key."),
        "http.network" => ("network", retry),
        "http.provider_busy" => ("busy", retry),
        "decision.api_status" => ("other", fix_input),
        "http.quota_exhausted" => ("quota", fix_input),
        "decision.cancelled" => ("cancelled", fix_input),
        "decision.response_parse" | "decision.schema_mismatch" => ("response_invalid", fix_input),
        s if s.starts_with("log.") => ("log", fix_input),
        s if s.starts_with("config.") => ("config", fix_input),
        _ => ("validation", fix_input),
    };
    Error::Failure {
        id,
        message: message.to_string(),
        category,
        action,
        attempts: None,
        session_id: None,
    }
}
fn contextual(mut failure: Error, context: impl std::fmt::Display, session: Option<&str>) -> Error {
    let Error::Failure {
        message,
        session_id,
        ..
    } = &mut failure;
    message.push_str(&format!("; {context}"));
    *session_id = session.map(str::to_owned).or_else(|| session_id.take());
    failure
}
pub fn result(value: Result<Value, Error>) -> Value {
    match value {
        Ok(value) => json!({"ok":true,"value":value}),
        Err(error) => json!({"ok":false,"error":error}),
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Runner {
    pub host: String,
    pub port: u16,
}
/// A configured model: the built-in provider it belongs to and the id sent on the wire.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ModelEntry {
    pub provider: String,
    pub id: String,
    #[serde(
        default,
        deserialize_with = "deserialize_model_release",
        skip_serializing_if = "Option::is_none"
    )]
    pub released: Option<String>,
}

fn deserialize_model_release<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    let date = String::deserialize(deserializer)?;
    serde_json::from_value::<sc_observability::Timestamp>(json!(format!("{date}T00:00:00Z")))
        .map_err(|e| DeError::custom(format!("released must be a YYYY-MM-DD date: {e}")))?;
    Ok(Some(date))
}
#[derive(Clone, Debug, Serialize)]
pub struct Settings {
    pub project: Option<String>,
    pub workers: usize,
    /// Configured model names; an entry wins over the built-in table.
    pub models: BTreeMap<String, ModelEntry>,
    pub local: BTreeMap<String, Runner>,
    pub max_sends: u32,
    pub decidr: BTreeMap<String, String>,
    #[serde(skip)]
    pub tool: String,
}
fn prefix(tool: &str) -> String {
    format!("{}_", tool.to_uppercase().replace('-', "_"))
}
fn layers(tool: &str) -> String {
    format!(
        "local {tool}.local.json, local {tool}.json, local sc-agent.json, global {tool}.local.json, global {tool}.json, or global sc-agent.json"
    )
}
fn missing_setting(tool: &str, path: &str) -> Error {
    error(
        "config.missing_setting",
        format!(
            "Set {path} through {}{}, {}",
            prefix(tool),
            path.to_uppercase().replace('.', "__"),
            layers(tool)
        ),
    )
}
fn setting<T: serde::de::DeserializeOwned>(
    tool: &str,
    resolved: &Resolved,
    path: &str,
    default: Value,
) -> Result<T, Error> {
    let selected = resolved
        .value
        .pointer(&format!("/{}", path.replace('.', "/")));
    if selected.is_none()
        && let Some((head, _)) = path.split_once('.')
        && PROVIDERS.iter().any(|p| p.url.is_empty() && p.name == head)
    {
        return Err(invalid_setting(resolved, head, missing_setting(tool, path)));
    }
    serde_json::from_value(selected.cloned().unwrap_or(default))
        .map_err(|e| invalid_setting(resolved, path, e))
}
impl Settings {
    pub fn project_required(&self) -> Result<&str, Error> {
        self.project
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                error(
                    "config.missing_project",
                    format!(
                        "Set project/phase through --project, SC_PROJECT, {}",
                        layers(&self.tool)
                    ),
                )
            })
    }
}
const KEYS: &[&str] = &[
    "project",
    "workers",
    "models",
    "max_sends",
    "decidr.luna-system-prompt",
    "decidr.haiku-system-prompt",
];
pub struct Configuration {
    pub config: Settings,
    pub rows: Vec<Value>,
    pub files: Vec<Value>,
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
pub fn config(
    tool: &str,
    local: Option<&Path>,
    global: Option<&Path>,
    environment: &[(OsString, OsString)],
    project: Option<&str>,
) -> Result<Configuration, Error> {
    let env_prefix = prefix(tool);
    let variables = environment
        .iter()
        .filter(|(name, _)| {
            let key = name
                .to_str()
                .and_then(|name| name.strip_prefix(env_prefix.as_str()))
                .and_then(|key| key.split("__").next())
                .unwrap_or("")
                .to_ascii_lowercase();
            !matches!(key.as_str(), "log_root" | "project")
        })
        .cloned()
        .collect();
    let mut keys = KEYS.iter().map(|key| key.to_string()).collect::<Vec<_>>();
    keys.extend(
        PROVIDERS
            .iter()
            .filter(|p| p.url.is_empty())
            .flat_map(|p| ["host", "port"].map(|key| format!("{}.{key}", p.name))),
    );
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
    let mut layers = StandardLayers::new(context).defaults(json!({"workers":8,"max_sends":5}));
    let mut files = Vec::new();
    for (slot, folder) in [(Slot::User, Some(global.as_path())), (Slot::Local, local)] {
        if let Some(folder) = folder {
            for name in [
                "sc-agent.json".into(),
                format!("{tool}.json"),
                format!("{tool}.local.json"),
            ] {
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
        .insert(
            Slot::Env,
            EnvSource::new(env_prefix, variables).alias("SC_PROJECT", ["project"]),
        )
        .flags(flags)
        .resolve()
        .map_err(config_error)?;
    let mut local = BTreeMap::new();
    for name in PROVIDERS
        .iter()
        .filter(|p| p.url.is_empty())
        .map(|p| p.name)
    {
        if resolved.value.get(name).is_some_and(Value::is_object) {
            let host = setting(tool, &resolved, &format!("{name}.host"), Value::Null)?;
            let port = setting(tool, &resolved, &format!("{name}.port"), Value::Null)?;
            local.insert(name.into(), Runner { host, port });
        }
    }
    let config = Settings {
        project: setting(tool, &resolved, "project", Value::Null)?,
        workers: setting(tool, &resolved, "workers", json!(8))?,
        models: setting(tool, &resolved, "models", json!({}))?,
        local,
        max_sends: setting(tool, &resolved, "max_sends", json!(5))?,
        decidr: setting(tool, &resolved, "decidr", json!({}))?,
        tool: tool.into(),
    };
    if config.workers == 0 {
        return Err(invalid_setting(&resolved, "workers", "must be positive"));
    }
    if config.max_sends == 0 {
        return Err(invalid_setting(&resolved, "max_sends", "must be positive"));
    }
    for entry in config.models.values() {
        provider_named(&entry.provider)?;
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
    files.reverse();
    Ok(Configuration {
        config,
        rows,
        files,
    })
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
pub fn log_root(name: &str) -> Result<PathBuf, Error> {
    std::env::var_os(format!("{}LOG_ROOT", prefix(name)))
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(|| {
            std::env::home_dir()
                .map(|p| p.join(".sc-agent"))
                .ok_or_else(|| error("config.home", "Home directory unavailable"))
        })
}
pub fn default_logger(name: &str) -> Result<Logger, Error> {
    slug(name)?;
    let mut config = LoggerConfig::default_for(
        ServiceName::new(name).map_err(|e| error("config.logger", e))?,
        log_root(name)?,
    );
    config.enable_console_sink = false;
    Logger::new(config).map_err(|e| error("config.logger", e))
}
#[derive(Debug)]
pub enum KeyringError {
    Read(String),
    Timeout,
}
impl From<String> for KeyringError {
    fn from(value: String) -> Self {
        Self::Read(value)
    }
}
pub fn key(
    variable: &str,
    environment: &[(OsString, OsString)],
    root: &Path,
    keyring: &dyn Fn(&str) -> Result<String, KeyringError>,
) -> Result<String, Error> {
    let env = environment
        .iter()
        .find(|(name, _)| name == variable)
        .map(|(_, v)| v.to_string_lossy().into_owned());
    let dotenv = || -> Result<Option<String>, Error> {
        let path = root.join(".env");
        if !path.exists() {
            return Ok(None);
        }
        for entry in dotenvy::from_path_iter(path).map_err(|e| error("config.dotenv", e))? {
            let (name, value) = entry.map_err(|e| error("config.dotenv", e))?;
            if name == variable {
                return Ok(Some(value));
            }
        }
        Ok(None)
    };
    let value = match env {
        Some(value) => value,
        None => match dotenv()? {
            Some(value) => value,
            None => keyring(variable).map_err(|e| match e {
                KeyringError::Read(detail) => error(
                    "secrets.keyring_read",
                    format!("The platform keyring is unavailable for {variable} : {detail}. Set {variable} in the environment or repository .env."),
                ),
                KeyringError::Timeout => error("secrets.keyring_timeout", format!("OS keychain did not answer for {variable}; it may be waiting for approval. Approve access or set {variable} in the environment or .env.")),
            })?,
        },
    };
    if value.is_empty() {
        return Err(error(
            "secrets.key_missing",
            format!(
                "{variable} was not found in the environment, repository .env, or platform keyring."
            ),
        ));
    }
    if !value.is_ascii() || value.bytes().any(|b| b <= 32 || b == 127) {
        return Err(error(
            "secrets.key_malformed",
            format!("{variable} contains whitespace, controls or non-ASCII characters"),
        ));
    }
    Ok(value)
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

#[cfg(test)]
mod resolve_test {
    use super::*;
    #[test]
    fn a_keyless_candidate_beats_a_keyed_one_and_is_never_looked_up() {
        let seam = |_: &str| -> Result<String, KeyringError> { Err(KeyringError::Timeout) };
        let options = Options {
            environment: vec![("ANTHROPIC_API_KEY".into(), "held".into())],
            settings: Settings {
                project: None,
                workers: 1,
                models: BTreeMap::new(),
                local: BTreeMap::new(),
                max_sends: 1,
                decidr: BTreeMap::new(),
                tool: String::new(),
            },
            model: String::new(),
            agent_role: String::new(),
            timeout: Duration::ZERO,
            base_url: None,
            root: PathBuf::new(),
            log_root: PathBuf::new(),
            repo: String::new(),
        };
        let keyed = || (&PROVIDERS[1], "keyed".to_owned());
        let keyless = || (&PROVIDERS[6], "keyless".to_owned());
        let picked = resolve(vec![keyed(), keyless()], &options, &seam);
        assert!(matches!(picked, Ok((p, id)) if p.key.is_empty() && id == "keyless"));
        let picked = resolve(vec![(&PROVIDERS[3], "x".into()), keyed()], &options, &seam);
        assert!(matches!(picked, Ok((p, _)) if p.name == "anthropic"));
    }
}
#[cfg(test)]
mod keyring_timeout_test {
    use super::*;
    #[test]
    fn nonresponding_reader_is_a_typed_timeout() {
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let value = keyring_wait(
            move || {
                let _ = wait.recv();
                Ok("unused".into())
            },
            Duration::ZERO,
        );
        let result = key(
            "TYPESAFE_API_KEY",
            &[],
            Path::new("/nonexistent"),
            &|_| match value {
                Err(KeyringError::Timeout) => Err(KeyringError::Timeout),
                _ => Err(KeyringError::Read("unexpected".into())),
            },
        );
        let _ = release.send(());
        let error = serde_json::to_value(result.err()).ok();
        assert_eq!(
            error
                .as_ref()
                .and_then(|e| e.get("category"))
                .and_then(Value::as_str),
            Some("auth")
        );
        assert!(
            error
                .as_ref()
                .and_then(|e| e.get("action"))
                .and_then(Value::as_str)
                .is_some_and(|s| s.contains("Approve"))
        );
        assert_eq!(
            error
                .as_ref()
                .and_then(|e| e.get("id"))
                .and_then(Value::as_str),
            Some("secrets.keyring_timeout")
        );
        assert!(
            error
                .as_ref()
                .and_then(|e| e.get("action"))
                .and_then(Value::as_str)
                .is_some_and(|s| s.contains(".env"))
        );
    }

    #[test]
    fn reader_failure_keeps_read_error() -> Result<(), Box<dyn std::error::Error>> {
        let value = keyring_wait(|| Err("controlled refusal".into()), Duration::from_secs(60));
        let result = key("TYPESAFE_API_KEY", &[], Path::new("/nonexistent"), &|_| {
            Err(match &value {
                Err(KeyringError::Read(message)) => KeyringError::Read(message.clone()),
                _ => KeyringError::Timeout,
            })
        });
        let error = serde_json::to_value(result.err()).ok();
        assert_eq!(
            error
                .as_ref()
                .and_then(|e| e.get("id"))
                .and_then(Value::as_str),
            Some("secrets.keyring_read")
        );
        let message = error
            .as_ref()
            .and_then(|e| e.get("message"))
            .and_then(Value::as_str)
            .ok_or("keyring read message absent")?;
        assert!(message.contains("platform keyring is unavailable"));
        assert!(message.contains("TYPESAFE_API_KEY"));
        assert!(message.contains(".env"));
        assert!(message.contains("controlled refusal"));
        Ok(())
    }

    #[test]
    fn missing_key_names_environment_dotenv_and_keyring() -> Result<(), Box<dyn std::error::Error>>
    {
        let value = keyring_result(Err(keyring::Error::NoEntry));
        assert!(matches!(&value, Ok(value) if value.is_empty()));
        let result = key("TYPESAFE_API_KEY", &[], Path::new("/nonexistent"), &|_| {
            value.clone().map_err(KeyringError::Read)
        });
        let error = serde_json::to_value(result.err()).ok();
        assert_eq!(
            error
                .as_ref()
                .and_then(|e| e.get("id"))
                .and_then(Value::as_str),
            Some("secrets.key_missing")
        );
        let message = error
            .as_ref()
            .and_then(|e| e.get("message"))
            .and_then(Value::as_str)
            .ok_or("missing-key message absent")?;
        assert!(message.contains("TYPESAFE_API_KEY"));
        assert!(message.contains("environment"));
        assert!(message.contains(".env"));
        assert!(message.contains("keyring"));
        Ok(())
    }

    #[test]
    fn unavailable_platform_keyring_has_env_and_dotenv_fallbacks()
    -> Result<(), Box<dyn std::error::Error>> {
        let errors = [
            keyring::Error::NoStorageAccess(Box::new(std::io::Error::other("denied"))),
            keyring::Error::PlatformFailure(Box::new(std::io::Error::other("missing service"))),
        ];
        for cause in errors {
            let detail = keyring_result(Err(cause))
                .err()
                .ok_or("keyring error lost")?;
            let result = key("TYPESAFE_API_KEY", &[], Path::new("/nonexistent"), &|_| {
                Err(KeyringError::Read(detail.clone()))
            });
            let error = serde_json::to_value(result.err())?;
            assert_eq!(error["id"], "secrets.keyring_read");
            let message = error["message"].as_str().ok_or("keyring message absent")?;
            assert!(message.contains("platform keyring is unavailable"));
            assert!(message.contains("TYPESAFE_API_KEY"));
            assert!(message.contains(".env"));
            assert!(message.contains(&detail), "{message}");
            assert!(
                error["action"]
                    .as_str()
                    .is_some_and(|action| action.contains(".env"))
            );
        }
        Ok(())
    }

    #[test]
    fn production_timeout_is_fifteen_seconds() {
        let source = include_str!("lib.rs");
        let production = source
            .split("pub fn os_keyring(")
            .nth(1)
            .and_then(|s| s.split("#[cfg(test)]").next());
        assert!(production.is_some_and(|s| s.contains("Duration::from_secs(15)")));
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
pub enum Wire {
    #[serde(rename = "systemone")]
    Jev,
    #[serde(rename = "chat/completions")]
    Chat,
    #[serde(rename = "messages")]
    Messages,
}
#[derive(Debug, Serialize)]
pub struct Provider {
    pub name: &'static str,
    pub url: &'static str,
    pub key: &'static str,
    pub wire: Wire,
    pub models: &'static [(&'static str, &'static str, &'static str)],
    pub open: bool,
}
const CLAUDE: &[(&str, &str, &str)] = &[
    ("fable", "claude-fable-5-2", "2026-11-09"),
    ("haiku", "claude-haiku-5-5", "2026-10-07"),
    ("sonnet", "claude-sonnet-5-5", "2026-09-28"),
    ("opus", "claude-opus-5-5", "2026-09-22"),
];
const GPT: &[(&str, &str, &str)] = &[
    ("terra", "gpt-6.1-terra", "2026-10-26"),
    ("sol", "gpt-6.1-sol", "2026-09-29"),
    ("luna", "gpt-6-luna", "2026-09-22"),
    ("astra", "gpt-6-astra", "2026-09-03"),
];
pub const TYPESAFE: &str = "typesafe";
pub static PROVIDERS: &[Provider] = &[
    Provider {
        name: TYPESAFE,
        url: "https://api.typesafe.ai/v1",
        key: "TYPESAFE_API_KEY",
        wire: Wire::Jev,
        models: &[("jev", "jev-latest", "2026-10-14")],
        open: false,
    },
    Provider {
        name: "anthropic",
        url: "https://api.anthropic.com/v1",
        key: "ANTHROPIC_API_KEY",
        wire: Wire::Messages,
        models: CLAUDE,
        open: false,
    },
    Provider {
        name: "openai",
        url: "https://api.openai.com/v1",
        key: "OPENAI_API_KEY",
        wire: Wire::Chat,
        models: GPT,
        open: false,
    },
    Provider {
        name: "openrouter",
        url: "https://openrouter.ai/api/v1",
        key: "OPENROUTER_API_KEY",
        wire: Wire::Chat,
        models: &[
            ("fable", "anthropic/claude-fable-5.2", "2026-11-09"),
            ("terra", "openai/gpt-6.1-terra", "2026-10-26"),
            ("haiku", "anthropic/claude-haiku-5.5", "2026-10-07"),
            ("sol", "openai/gpt-6.1-sol", "2026-09-29"),
            ("sonnet", "anthropic/claude-sonnet-5.5", "2026-09-28"),
            ("opus", "anthropic/claude-opus-5.5", "2026-09-22"),
            ("luna", "openai/gpt-6-luna", "2026-09-22"),
            ("astra", "openai/gpt-6-astra", "2026-09-03"),
        ],
        open: true,
    },
    Provider {
        name: "together",
        url: "https://api.together.ai/v1",
        key: "TOGETHER_API_KEY",
        wire: Wire::Chat,
        models: &[
            ("fable", "anthropic/claude-fable-5-2", "2026-11-09"),
            ("terra", "openai/gpt-6.1-terra", "2026-10-26"),
            ("haiku", "anthropic/claude-haiku-5-5", "2026-10-07"),
            ("sol", "openai/gpt-6.1-sol", "2026-09-29"),
            ("sonnet", "anthropic/claude-sonnet-5-5", "2026-09-28"),
            ("opus", "anthropic/claude-opus-5-5", "2026-09-22"),
            ("luna", "openai/gpt-6-luna", "2026-09-22"),
            ("astra", "openai/gpt-6-astra", "2026-09-03"),
        ],
        open: true,
    },
    Provider {
        name: "nous",
        url: "https://inference-api.nousresearch.com/v1",
        key: "NOUS_API_KEY",
        wire: Wire::Chat,
        models: &[
            ("haiku", "anthropic/claude-haiku-5.5", "2026-10-07"),
            ("luna", "openai/gpt-6-luna", "2026-09-22"),
        ],
        open: true,
    },
    Provider {
        name: "laya",
        url: "",
        key: "",
        wire: Wire::Jev,
        models: &[("laya", "laya:en", "2026-09-23")],
        open: false,
    },
    Provider {
        name: "decider_4b",
        url: "",
        key: "",
        wire: Wire::Jev,
        models: &[("decider", "decider-4b-v2.1", "2026-09-24")],
        open: false,
    },
    Provider {
        name: "decider_2b",
        url: "",
        key: "",
        wire: Wire::Jev,
        models: &[("decider", "decider-2b-v11", "2026-09-24")],
        open: false,
    },
];
fn provider_named(name: &str) -> Result<&'static Provider, Error> {
    PROVIDERS
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| error("types.unknown_provider", name))
}
/// Resolve a model name to its first configured or catalog candidate.
pub fn model_with(
    name: &str,
    configured: &BTreeMap<String, ModelEntry>,
) -> Result<(&'static Provider, String), Error> {
    Ok(candidates(name, configured)?.remove(0))
}
/// Every provider row that serves `name`, newest release first, equal dates in table order.
fn candidates(
    name: &str,
    configured: &BTreeMap<String, ModelEntry>,
) -> Result<Vec<(&'static Provider, String)>, Error> {
    if let Some(entry) = configured.get(name) {
        let p = provider_named(&entry.provider)?;
        return Ok(vec![(p, entry.id.clone())]);
    }
    let mut rows = Vec::new();
    for p in PROVIDERS {
        for (_, id, date) in p.models.iter().filter(|(s, i, _)| *s == name || *i == name) {
            rows.push((*date, p, (*id).to_owned()));
        }
    }
    rows.sort_by_key(|row| std::cmp::Reverse(row.0));
    let found = rows
        .into_iter()
        .map(|(_, p, id)| (p, id))
        .collect::<Vec<_>>();
    if found.is_empty() {
        return Err(error("types.unknown_short_name", name));
    }
    Ok(found)
}
/// Pick the candidate to send to: a keyless provider first, else the first whose key is held.
fn resolve(
    found: Vec<(&'static Provider, String)>,
    options: &Options,
    keyring: &(dyn Fn(&str) -> Result<String, KeyringError> + Sync),
) -> Result<(&'static Provider, String), Error> {
    let single = found.first().filter(|_| found.len() == 1);
    if let Some(row) = found.iter().find(|(p, _)| p.key.is_empty()).or(single) {
        return Ok(row.clone());
    }
    let mut last = None;
    for (p, id) in &found {
        match key(p.key, &options.environment, &options.root, keyring) {
            Ok(_) => return Ok((p, id.clone())),
            Err(mut failure) => {
                if let Some(mut previous) = last.take() {
                    // Missing keys add context without replacing an earlier failure.
                    let Error::Failure { id, .. } = &failure;
                    if *id == "secrets.key_missing" {
                        std::mem::swap(&mut previous, &mut failure);
                    }
                    failure = contextual(failure, previous, None);
                }
                last = Some(failure);
            }
        }
    }
    Err(last.unwrap_or_else(|| error("types.unknown_short_name", &options.model)))
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
    fn check_shape(value: &Value, shape: &Value, path: &str) -> Result<(), Error> {
        if let Some(values) = shape.get("enum").and_then(Value::as_array)
            && !values.contains(value)
        {
            return Err(error(
                "decision.schema_mismatch",
                format!("{path}: value is outside requested choices"),
            ));
        }
        if let Some(properties) = shape.get("properties").and_then(Value::as_object)
            && !value.is_null()
        {
            let fields = value.as_object().ok_or_else(|| {
                error(
                    "decision.schema_mismatch",
                    format!("{path}: expected an object"),
                )
            })?;
            for key in shape
                .get("required")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                if !fields.contains_key(key) {
                    return Err(error(
                        "decision.schema_mismatch",
                        format!("{path}.{key}: required field is missing"),
                    ));
                }
            }
            for (key, child) in fields {
                if let Some(rule) = properties.get(key) {
                    check_shape(child, rule, &format!("{path}.{key}"))?;
                } else if shape.get("additionalProperties").and_then(Value::as_bool) == Some(false)
                {
                    return Err(error(
                        "decision.schema_mismatch",
                        format!("{path}.{key}: unexpected field"),
                    ));
                }
            }
        }
        Ok(())
    }
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
        check_shape(answer, &answer_schema(question), id)?;
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

#[derive(Clone)]
pub struct Options {
    pub environment: Vec<(OsString, OsString)>,
    pub settings: Settings,
    pub model: String,
    pub agent_role: String,
    pub timeout: Duration,
    pub base_url: Option<String>,
    pub root: PathBuf,
    pub log_root: PathBuf,
    pub repo: String,
}
pub fn log(logger: &Logger, action: &str, fields: Value) -> Result<(), Error> {
    let event = serde_json::from_value(
        json!({"version":"v1","timestamp":sc_observability::Timestamp::now_utc(),"level":if action=="error" {sc_observability::Level::Error}else{sc_observability::Level::Info},"service":logger.service_name(),"target":"sc-agent","action":action,"identity":sc_observability::ProcessIdentity::default(),"fields":fields}),
    ).map_err(|e| error("log.invalid_event", e))?;
    logger.log(event).map_err(|e| error("log.write", e))
}

fn answer_schema(question: &Value) -> Value {
    let kind = question.get("type").and_then(Value::as_str).unwrap_or("");
    let names = match question.get("criteria") {
        Some(Value::Object(values)) if kind == "choice" => values.keys().cloned().collect(),
        Some(Value::Array(values)) if kind != "choice" => (0..values.len())
            .map(|index| index.to_string())
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    let object = |value_type| {
        json!({
            "type":"object",
            "properties":names.iter().map(|name|(name.clone(),json!({"type":value_type}))).collect::<BTreeMap<_,_>>(),
            "required":names,
            "additionalProperties":false
        })
    };
    let mut fields = serde_json::Map::new();
    fields.insert("type".into(), json!({"type":"string","enum":[kind]}));
    match kind {
        "choice" | "score" => {
            fields.insert("confidence".into(), json!({"type":"number"}));
            fields.insert("probabilities".into(), object("number"));
            if kind == "choice" {
                fields.insert("choice".into(), json!({"type":"string","enum":names}));
            } else {
                fields.insert("score".into(), json!({"type":"number"}));
                fields.insert("legend".into(), object("string"));
            }
        }
        "box" => {
            fields.insert("box".into(),json!({
                    "type":["object","null"],
                    "properties":(["top","bottom","left","right"].into_iter().map(|name|(name,json!({"type":"number"}))).collect::<BTreeMap<_,_>>()),
                    "required":["top","bottom","left","right"],"additionalProperties":false
                }));
            fields.insert("confidence".into(), json!({"type":["number","null"]}));
        }
        _ => {
            fields.insert("noul".into(), json!({"type":"number"}));
        }
    }
    let required = fields.keys().cloned().collect::<Vec<_>>();
    json!({"type":"object","properties":fields,"required":required,"additionalProperties":false})
}
pub type DecisionRequest = Value;
pub type DecisionResult = Value;
#[derive(Debug)]
pub struct LlmRequest {
    pub system: Option<String>,
    pub messages: Value,
    pub schema: Option<Value>,
}
pub trait Decision {
    fn decide(&self, req: &DecisionRequest) -> Result<DecisionResult, Error>;
}
pub trait Llm {
    fn call(&self, req: &LlmRequest) -> Result<Value, Error>;
}
pub const DEFAULT_SYSTEM_PROMPT: &str = "Answer every question in the decision request. Reply with one JSON object that matches the supplied schema, including probabilities and confidence.";
type Auth = &'static [(&'static str, &'static str)];
const BEARER: Auth = &[("Authorization", "Bearer {}")];
const ANTHROPIC: Auth = &[("x-api-key", "{}"), ("anthropic-version", "2023-06-01")];
// Messages model output limit: https://platform.claude.com/docs/en/models/overview
const ANTHROPIC_MAX_TOKENS: u32 = 128000;
pub struct Holdoffs {
    started: Instant,
    deadlines: Mutex<BTreeMap<String, Duration>>,
}
impl Default for Holdoffs {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            deadlines: Mutex::new(BTreeMap::new()),
        }
    }
}
pub struct Route<'a> {
    pub options: &'a Options,
    pub provider: &'static Provider,
    pub model: String,
    pub base: String,
    pub cancel: &'a Cancel,
    pub wait: &'a (dyn Fn(Duration) -> Result<(), Error> + Sync),
    pub keyring: &'a (dyn Fn(&str) -> Result<String, KeyringError> + Sync),
    pub sends: &'a Cell<u32>,
    pub holdoffs: &'a Holdoffs,
}
pub struct Jev<'a>(pub &'a Route<'a>);
pub struct OpenAiCompat<'a>(pub &'a Route<'a>);
pub struct AnthropicCompat<'a>(pub &'a Route<'a>);
fn ask(llm: &impl Llm, route: &Route<'_>, req: &DecisionRequest) -> Result<DecisionResult, Error> {
    let decidr = &route.options.settings.decidr;
    let short = route
        .provider
        .models
        .iter()
        .find(|row| row.1 == route.model)
        .map_or(route.options.model.as_str(), |row| row.0);
    let prompt = decidr
        .get(&format!("{}-system-prompt", route.options.model))
        .or_else(|| decidr.get(&format!("{short}-system-prompt")));
    let questions = req
        .get("questions")
        .and_then(Value::as_object)
        .into_iter()
        .flatten();
    let answers = questions
        .map(|(id, question)| (id, answer_schema(question)))
        .collect::<BTreeMap<_, _>>();
    let reply = llm.call(&LlmRequest {
        system: Some(prompt.map_or(DEFAULT_SYSTEM_PROMPT, String::as_str).into()),
        messages: json!([{"role":"user","content":req.to_string()}]),
        schema: Some(json!({
            "type":"object", "required":["answers"], "additionalProperties":false,
            "properties":{"answers":{"type":"object","properties":answers,"required":answers.keys().collect::<Vec<_>>(),"additionalProperties":false}}
        })),
    })?;
    schema(req, &reply).map(|()| reply)
}
impl Decision for Jev<'_> {
    fn decide(&self, req: &DecisionRequest) -> Result<DecisionResult, Error> {
        let mut body = req.clone();
        if let Some(body) = body.as_object_mut() {
            body.insert("model".into(), json!(self.0.model));
        }
        let reply = self.0.post("systemone", BEARER, &body)?;
        schema(req, &reply).map(|()| reply)
    }
}
impl Decision for OpenAiCompat<'_> {
    fn decide(&self, req: &DecisionRequest) -> Result<DecisionResult, Error> {
        ask(self, self.0, req)
    }
}
impl Decision for AnthropicCompat<'_> {
    fn decide(&self, req: &DecisionRequest) -> Result<DecisionResult, Error> {
        ask(self, self.0, req)
    }
}
impl Llm for OpenAiCompat<'_> {
    fn call(&self, req: &LlmRequest) -> Result<Value, Error> {
        let system = req
            .system
            .iter()
            .map(|text| json!({"role":"system","content":text}));
        let messages = system
            .chain(req.messages.as_array().cloned().unwrap_or_default())
            .collect::<Vec<_>>();
        let format = req.schema.as_ref().map(|schema| json!({"type":"json_schema","json_schema":{"name":"decision","strict":true,"schema":schema}}));
        let body = json!({"model":self.0.model,"messages":messages,"response_format":format});
        self.0.text(
            "chat/completions",
            BEARER,
            body,
            "/choices/0/message/content",
        )
    }
}
impl Llm for AnthropicCompat<'_> {
    fn call(&self, req: &LlmRequest) -> Result<Value, Error> {
        let format = req
            .schema
            .as_ref()
            .map(|schema| json!({"format":{"type":"json_schema","schema":schema}}));
        let body = json!({"model":self.0.model,"max_tokens":ANTHROPIC_MAX_TOKENS,"messages":req.messages,"system":req.system,"output_config":format});
        self.0.text("messages", ANTHROPIC, body, "/content")
    }
}
impl Route<'_> {
    fn defer(&self, host: &str, now: Duration, delay: Duration) -> Result<Duration, Error> {
        let mut holdoffs = self.cancel.lock(&self.holdoffs.deadlines)?;
        let deadline = holdoffs.entry(host.to_owned()).or_default();
        *deadline = (*deadline).max(now.saturating_add(delay));
        Ok(*deadline)
    }
    fn holdoff(&self, host: &str, elapsed: &mut Duration) -> Result<(), Error> {
        loop {
            self.cancel.check()?;
            let deadline = self
                .cancel
                .lock(&self.holdoffs.deadlines)?
                .get(host)
                .copied()
                .unwrap_or_default();
            let delay = deadline.saturating_sub(*elapsed);
            if delay.is_zero() {
                return Ok(());
            }
            (self.wait)(delay)?;
            // A completed injected wait reaches this deadline, including simulated waits.
            *elapsed = deadline.max(self.holdoffs.started.elapsed());
        }
    }
    fn agent(&self) -> ureq::Agent {
        ureq::Agent::config_builder()
            .max_redirects(0)
            .http_status_as_error(false)
            .timeout_global(Some(self.options.timeout))
            .build()
            .new_agent()
    }
    fn text(&self, path: &str, auth: Auth, mut body: Value, at: &str) -> Result<Value, Error> {
        if let Some(fields) = body.as_object_mut() {
            fields.retain(|_, field| !field.is_null());
        }
        let reply = self.post(path, auth, &body)?;
        let text = reply.pointer(at).and_then(|value| {
            value.as_str().or_else(|| {
                value
                    .as_array()?
                    .iter()
                    .find_map(|block| block.get("text")?.as_str())
            })
        });
        let text = text.ok_or_else(|| {
            error(
                "decision.response_parse",
                format!("provider reply has no text: {reply}"),
            )
        })?;
        serde_json::from_str(text)
            .map_err(|e| error("decision.response_parse", format!("{e}: {text}")))
    }
    fn post(&self, path: &str, auth: Auth, body: &Value) -> Result<Value, Error> {
        let (cancel, provider, max) = (self.cancel, self.provider, self.options.settings.max_sends);
        let url = format!("{}/{path}", self.base.trim_end_matches('/'));
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
        if !provider.key.is_empty() && uri.scheme_str() != Some("https") && !loopback {
            return Err(error(
                "http.key_on_plain_http",
                format!(
                    "{} is never sent to the plain-http host {host}",
                    provider.key
                ),
            ));
        }
        let key = match provider.key {
            "" => String::new(),
            name => key(
                name,
                &self.options.environment,
                &self.options.root,
                self.keyring,
            )?,
        };
        let redact = |text: String| {
            if key.is_empty() {
                text
            } else {
                text.replace(&key, "[redacted]")
            }
        };
        let agent = self.agent();
        let mut elapsed = self.holdoffs.started.elapsed();
        let mut previous = None;
        for attempt in 1..=max {
            self.holdoff(&host, &mut elapsed)
                .map_err(|e| match previous.take() {
                    Some(original) if !cancel.is_cancelled() => contextual(original, e, None),
                    _ => e,
                })?;
            cancel.check()?;
            let mut post = agent.post(&url).header("Content-Type", "application/json");
            for (name, value) in auth.iter().filter(|_| !key.is_empty()) {
                post = post.header(*name, value.replace("{}", &key));
            }
            self.sends.set(attempt);
            // A send in flight cannot be aborted, so on cancel it is abandoned and the request ends now.
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            let sending = body.clone();
            std::thread::spawn(move || {
                let _ = sender.send(post.send_json(&sending).map(|mut reply| {
                    let delay = reply
                        .headers()
                        .get("Retry-After")
                        .and_then(|h| h.to_str().ok())
                        .and_then(|h| h.parse::<f64>().ok())
                        .and_then(|s| Duration::try_from_secs_f64(s).ok());
                    (
                        reply.status().as_u16(),
                        delay,
                        reply
                            .body_mut()
                            .with_config()
                            .lossy_utf8(true)
                            .read_to_string(),
                    )
                }));
            });
            let (status, delay, text) = match cancel.recv(&receiver, "http.network")? {
                Ok((status, delay, Ok(text))) => (status, delay, redact(text)),
                Ok((_, _, Err(e))) | Err(e) => (0, None, redact(e.to_string())),
            };
            if (200..300).contains(&status) {
                return serde_json::from_str(&text)
                    .map_err(|e| error("decision.response_parse", format!("{e}: {text}")));
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
            let id = match status {
                _ if quota => "http.quota_exhausted",
                _ if busy => "http.provider_busy",
                0 => "http.network",
                401 => "http.auth",
                _ => "decision.api_status",
            };
            let delay =
                delay.unwrap_or_else(|| Duration::from_secs(2u64.saturating_pow(attempt - 1)));
            elapsed = elapsed.max(self.holdoffs.started.elapsed());
            let deadline = if busy {
                Some(
                    self.defer(&host, elapsed, delay)
                        .map_err(|e| contextual(error(id, text.clone()), e, None))?,
                )
            } else {
                None
            };
            if (busy || status == 0) && attempt < max {
                let delay =
                    deadline.map_or(delay, |until| delay.max(until.saturating_sub(elapsed)));
                (self.wait)(delay).map_err(|e| contextual(error(id, text.clone()), e, None))?;
                elapsed = elapsed
                    .saturating_add(delay)
                    .max(self.holdoffs.started.elapsed());
                previous = Some(error(id, text));
                continue;
            }
            return Err(error(id, text));
        }
        Err(error("http.network", "send count exhausted"))
    }
}
static SESSION_APPEND: Mutex<()> = Mutex::new(());

pub fn workers(value: i64) -> Result<usize, Error> {
    usize::try_from(value)
        .ok()
        .filter(|v| *v > 0)
        .ok_or_else(|| error("decidr.invalid_workers", "workers must be positive"))
}

fn after_cancel(outcome: Result<Value, Error>, cancel: &Cancel) -> Result<Value, Error> {
    outcome.and_then(|value| cancel.check().map(|()| value))
}
type KeyringFn = dyn Fn(&str) -> Result<String, KeyringError> + Send + Sync;
pub struct Run {
    pub options: Options,
    pub cancel: Arc<Cancel>,
    pub wait: Box<dyn Fn(Duration) -> Result<(), Error> + Send + Sync>,
    pub keyring: Box<KeyringFn>,
    holdoffs: Holdoffs,
    found: Vec<(&'static Provider, String)>,
    logger: Arc<Logger>,
    owned: bool,
}
#[derive(Default)]
pub struct RunArgs {
    pub tool: String,
    pub agent_role: String,
    pub model: Option<String>,
    pub project: Option<String>,
    pub config_dir: Option<PathBuf>,
    pub timeout_secs: Option<f64>,
    pub workers: Option<i64>,
    pub base_url: Option<String>,
}
impl Run {
    pub fn new(
        args: RunArgs,
        logger: Option<Arc<Logger>>,
        cancel: Arc<Cancel>,
    ) -> Result<Self, Error> {
        let (loaded, root, repo) = load(
            &args.tool,
            args.config_dir.as_deref(),
            args.project.as_deref(),
        )?;
        let mut settings = loaded.config;
        if let Some(count) = args.workers {
            settings.workers = workers(count)?;
        }
        let options = Options {
            environment: std::env::vars_os().collect(),
            settings,
            model: args.model.unwrap_or_else(|| "jev".into()),
            agent_role: args.agent_role,
            timeout: Duration::try_from_secs_f64(args.timeout_secs.unwrap_or(120.0))
                .map_err(|e| error("decidr.invalid_request_timeout", e))?,
            base_url: args.base_url,
            root,
            log_root: log_root(&args.tool)?,
            repo,
        };
        Self::from_options(options, args.tool, logger, cancel)
    }
    pub fn from_options(
        options: Options,
        tool: String,
        logger: Option<Arc<Logger>>,
        cancel: Arc<Cancel>,
    ) -> Result<Self, Error> {
        let found = candidates(&options.model, &options.settings.models)?;
        let owned = logger.is_none();
        let logger = match logger {
            Some(logger) => logger,
            None => Arc::new(default_logger(&tool)?),
        };
        let waiting = cancel.clone();
        Ok(Self {
            options,
            cancel,
            wait: Box::new(move |delay| waiting.wait(delay)),
            keyring: Box::new(os_keyring),
            holdoffs: Holdoffs::default(),
            found,
            logger,
            owned,
        })
    }
    fn resolve(&self) -> Result<(&'static Provider, String), Error> {
        resolve(self.found.clone(), &self.options, &*self.keyring)
    }
    /// Whether the next send resolves to the named provider (keyless first, then the first held key).
    pub fn sends_to(&self, provider: &str) -> Result<bool, Error> {
        self.resolve().map(|(p, _)| p.name == provider)
    }
    pub fn logger(&self) -> &Logger {
        &self.logger
    }
    pub fn close(&self) -> Result<(), Error> {
        let flushed = self.logger.flush().map_err(|e| error("log.flush", e));
        let stopped = if self.owned {
            self.logger.shutdown().map_err(|e| error("log.shutdown", e))
        } else {
            Ok(())
        };
        match (flushed, stopped) {
            (Err(original), Err(later)) => Err(contextual(original, later, None)),
            (Err(original), _) => Err(original),
            (_, stopped) => stopped,
        }
    }
    pub fn send(&self, request: &Value) -> Result<Value, Error> {
        let (options, logger, cancel) = (&self.options, self.logger(), &*self.cancel);
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
        let sends = Cell::new(0);
        let outcome = (|| {
            cancel.check()?;
            let project = options.settings.project_required()?;
            slug(project)?;
            slug(&options.agent_role)?;
            let (provider, model) = self.resolve()?;
            let base = match options.settings.local.get(provider.name) {
                _ if !provider.url.is_empty() => provider.url.into(),
                Some(runner) => format!("http://{}:{}/v1", runner.host, runner.port),
                None => {
                    return Err(missing_setting(
                        &options.settings.tool,
                        &format!("{}.host", provider.name),
                    ));
                }
            };
            let route = Route {
                options,
                provider,
                model,
                base: options.base_url.clone().unwrap_or(base),
                cancel,
                wait: &*self.wait,
                keyring: &*self.keyring,
                sends: &sends,
                holdoffs: &self.holdoffs,
            };
            match provider.wire {
                Wire::Messages => AnthropicCompat(&route).decide(request),
                Wire::Chat => OpenAiCompat(&route).decide(request),
                Wire::Jev => Jev(&route).decide(request),
            }
        })();
        let mut outcome = after_cancel(outcome, cancel);
        if let Err(Error::Failure {
            session_id: id,
            attempts,
            ..
        }) = &mut outcome
        {
            *id = Some(session_id.clone());
            *attempts = (sends.get() > 0 || cancel.is_cancelled()).then(|| sends.get());
        }
        if let Ok(value) = &mut outcome
            && let Some(fields) = value.as_object_mut()
        {
            fields.insert("attempts".into(), json!(sends.get()));
            fields.insert("session_id".into(), json!(session_id));
            fields.insert("question_digests".into(), json!(digests));
        }
        let attach = |failure, context| contextual(failure, context, Some(&session_id));
        if let Ok(value) = &outcome
            && let Ok(project) = options.settings.project_required()
            && slug(project).is_ok()
            && slug(&options.agent_role).is_ok()
        {
            let row = json!({"session_id":session_id,"repo":options.repo,"agent_role":options.agent_role,"project":project,"request":request,"question_digests":digests,"result":{"ok":true,"value":value}});
            let path = options
                .log_root
                .join("sessions")
                .join(&options.repo)
                .join(&options.agent_role)
                .join(project);
            let written = (|| -> Result<(), Error> {
                let _append = SESSION_APPEND
                    .lock()
                    .map_err(|e| error("config.session", e))?;
                (|| -> std::io::Result<()> {
                    use std::io::Write;
                    std::fs::create_dir_all(&path)?;
                    writeln!(
                        std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(path.join("sessions.jsonl"))?,
                        "{row}"
                    )
                })()
                .map_err(|e| error("log.session_write", e))
            })();
            if let Err(failure) = written {
                outcome = Err(attach(failure, format!("original outcome: {outcome:?}")));
            }
        }
        let logged = match &outcome {
            Ok(value) => log(logger, "decision.result", json!({"response":value})),
            Err(_) => Ok(()),
        };
        for failure in [logged, logger.flush().map_err(|e| error("log.flush", e))]
            .into_iter()
            .filter_map(Result::err)
        {
            let context = format!("original outcome: {outcome:?}");
            outcome = Err(match outcome {
                Err(original) => attach(original, failure.to_string()),
                Ok(_) => attach(failure, context),
            });
        }
        outcome
    }

    pub fn send_batch(
        &self,
        requests: &[Value],
        on_result: &(dyn Fn(usize, &Value) + Sync),
    ) -> Result<Value, Error> {
        if self.options.settings.workers == 0 {
            return Err(error("decidr.invalid_workers", "workers must be positive"));
        }
        let next = std::sync::atomic::AtomicUsize::new(0);
        let (sender, receiver) = std::sync::mpsc::channel();
        Ok(std::thread::scope(|scope| {
            for _ in 0..self.options.settings.workers.min(requests.len()) {
                let sender = sender.clone();
                let next = &next;
                scope.spawn(move || {
                    loop {
                        let index = next.fetch_add(1, Ordering::SeqCst);
                        let Some(request) = requests.get(index) else {
                            break;
                        };
                        let value = report(self.logger(), self.send(request));
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
pub fn repository() -> Result<(PathBuf, String), Error> {
    let cwd = std::env::current_dir().map_err(|e| error("config.repo", e))?;
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let root = git(&["rev-parse", "--show-toplevel"])
        .map(PathBuf::from)
        .unwrap_or(cwd);
    let origin = git(&["remote", "get-url", "origin"]);
    let host = hostname(std::process::Command::new("hostname").output())?;
    let name = repository_identity(origin.as_deref(), &host, &root)?;
    Ok((root, name))
}
fn hostname(output: std::io::Result<std::process::Output>) -> Result<String, Error> {
    let output =
        output.map_err(|e| error("config.repo", format!("host name command failed: {e}")))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(error(
            "config.repo",
            format!("host name command failed with {}: {detail}", output.status),
        ));
    }
    let host = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!host.is_empty())
        .then_some(host)
        .ok_or_else(|| error("config.repo", "host name command returned an empty name"))
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

pub fn load(
    tool: &str,
    config_dir: Option<&Path>,
    project: Option<&str>,
) -> Result<(Configuration, PathBuf, String), Error> {
    let (root, repo) = repository()?;
    let local = root.join(".sc/sc-agent");
    let environment = std::env::vars_os().collect::<Vec<_>>();
    let local = root.join(".git").exists().then_some(local.as_path());
    Ok((
        config(tool, local, config_dir, &environment, project)?,
        root,
        repo,
    ))
}
pub fn doctor(
    tool: &str,
    config_dir: Option<&Path>,
    project: Option<&str>,
) -> Result<Value, Error> {
    let (loaded, ..) = load(tool, config_dir, project)?;
    Ok(json!({"settings":loaded.rows, "files":loaded.files, "log_root":log_root(tool)?}))
}

pub fn report(logger: &Logger, outcome: Result<Value, Error>) -> Value {
    let logged = match &outcome {
        Err(original) => log(logger, "error", json!(original)),
        Ok(_) => Ok(()),
    };
    after_failure(result(outcome), logged, "log.write")
}

#[cfg(test)]
extern crate self as sc_agent;
#[cfg(test)]
pub mod route_tests {
    use super::*;
    pub mod test_support {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/mod.rs"));
    }
    use test_support::*;
    #[test]
    fn resolution_failure_keeps_every_candidate_error() -> Test {
        for (case, expected_id) in [
            (0, "secrets.key_malformed"),
            (1, "secrets.keyring_read"),
            (2, "secrets.key_missing"),
        ] {
            let root = Temp::new()?;
            let mut options = options(&root, "http://127.0.0.1:1/v1");
            options.environment.clear();
            let lookup = |variable: &str| {
                if case == 2 {
                    return Ok(String::new());
                }
                match variable {
                    "ANTHROPIC_API_KEY" => Err(KeyringError::Timeout),
                    "OPENROUTER_API_KEY" => Err(KeyringError::Read("controlled refusal".into())),
                    "NOUS_API_KEY" if case == 0 => Ok("malformed key".into()),
                    _ => Ok(String::new()),
                }
            };
            let found = candidates("haiku", &BTreeMap::new())?;
            let causes = found
                .iter()
                .map(|(provider, _)| {
                    key(provider.key, &options.environment, &options.root, &lookup)
                        .err()
                        .ok_or("candidate unexpectedly has a key")
                        .map(|failure| failure.to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            let failure = result(resolve(found, &options, &lookup).map(|_| json!(null)));
            assert_eq!(failure["error"]["id"], expected_id);
            let message = failure["error"]["message"]
                .as_str()
                .ok_or("missing error message")?;
            let returned = format!(
                "{}: {message}",
                failure["error"]["id"].as_str().ok_or("missing error id")?
            );
            for cause in causes {
                assert!(returned.contains(&cause), "lost cause {cause}: {returned}");
            }
        }
        Ok(())
    }

    #[test]
    fn run_new_rejects_unknown_model_and_send_resolves_per_call() -> Test {
        let root = Temp::new()?;
        let logger = logger(&root)?;
        let args = RunArgs {
            tool: "decidr".into(),
            agent_role: "test".into(),
            project: Some("test".into()),
            config_dir: Some(root.0.clone()),
            model: Some("missing-contract-model".into()),
            ..Default::default()
        };
        let failure = Run::new(args, Some(logger.clone()), Arc::new(Cancel::default()))
            .err()
            .ok_or("unknown model accepted")?;
        assert_eq!(
            result(Err(failure))["error"]["id"],
            "types.unknown_short_name"
        );
        let reply = answer().to_string();
        let stub = Stub::new(vec![
            http(200, json!({"content":[{"text":reply}]})),
            http(200, json!({"choices":[{"message":{"content":reply}}]})),
        ])?;
        let mut options = options(&root, &stub.url);
        options.model = "haiku".into();
        options.environment.clear();
        let mut run = run(options, &logger)?;
        let second = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let selected = second.clone();
        run.keyring = Box::new(move |name| {
            Ok(
                if name
                    == (if selected.load(Ordering::SeqCst) {
                        "OPENROUTER_API_KEY"
                    } else {
                        "ANTHROPIC_API_KEY"
                    })
                {
                    "test-secret".into()
                } else {
                    String::new()
                },
            )
        });
        assert!(run.sends_to("anthropic")?);
        assert_eq!(run.send(&request())?["answers"], answer()["answers"]);
        second.store(true, Ordering::SeqCst);
        assert!(run.sends_to("openrouter")?);
        assert_eq!(run.send(&request())?["answers"], answer()["answers"]);
        assert_eq!(stub.finish()?.len(), 2);
        logger.shutdown()?;
        Ok(())
    }
    #[test]
    fn run_close_shuts_down_only_an_owned_logger() -> Test {
        for owned in [false, true] {
            let root = Temp::new()?;
            let logger = logger(&root)?;
            let mut run = run(options(&root, "http://127.0.0.1:1/v1"), &logger)?;
            assert!(!run.owned);
            // Exercise both ownership branches with a test-owned logger and no default-path writes.
            run.owned = owned;
            run.close()?;
            assert_eq!(logger.flush().is_err(), owned);
            if !owned {
                logger.shutdown()?;
            }
        }
        Ok(())
    }
    #[test]
    fn run_timeout_and_workers_apply_to_every_call() -> Test {
        let root = Temp::new()?;
        let logger = logger(&root)?;
        let args = RunArgs {
            tool: "decidr".into(),
            agent_role: "test".into(),
            project: Some("test".into()),
            config_dir: Some(root.0.clone()),
            timeout_secs: Some(30.0),
            workers: Some(2),
            ..Default::default()
        };
        let run = Run::new(args, Some(logger.clone()), Arc::new(Cancel::default()))?;
        assert_eq!(run.options.settings.workers, 2);
        let (provider, model) = model_with("jev", &BTreeMap::new())?;
        let sends = Cell::new(0);
        for _ in 0..2 {
            let route = Route {
                options: &run.options,
                provider,
                model: model.clone(),
                base: String::new(),
                cancel: &run.cancel,
                wait: &*run.wait,
                keyring: &keyring,
                sends: &sends,
                holdoffs: &Holdoffs::default(),
            };
            assert_eq!(
                route.agent().config().timeouts().global,
                Some(Duration::from_secs(30))
            );
        }
        let defaults = options(&root, "http://127.0.0.1:1/v1");
        let configured = defaults.settings.workers;
        let next = self::super::Run::from_options(
            defaults,
            "decidr".into(),
            Some(logger.clone()),
            Arc::new(Cancel::default()),
        )?;
        assert_eq!(next.options.settings.workers, configured);
        assert_eq!(next.options.timeout, Duration::from_secs(5));
        logger.shutdown()?;
        Ok(())
    }
    #[test]
    fn run_with_invalid_timeout_or_workers_is_an_error() -> Test {
        let root = Temp::new()?;
        let logger = logger(&root)?;
        for (timeout, workers, id) in [
            (Some(-1.0), None, "decidr.invalid_request_timeout"),
            (Some(f64::NAN), None, "decidr.invalid_request_timeout"),
            (None, Some(0), "decidr.invalid_workers"),
            (None, Some(-1), "decidr.invalid_workers"),
        ] {
            let args = RunArgs {
                tool: "decidr".into(),
                agent_role: "test".into(),
                project: Some("test".into()),
                config_dir: Some(root.0.clone()),
                timeout_secs: timeout,
                workers,
                ..Default::default()
            };
            let failure = Run::new(args, Some(logger.clone()), Arc::new(Cancel::default()))
                .err()
                .ok_or("invalid override accepted")?;
            assert_eq!(result(Err(failure))["error"]["id"], id);
        }
        logger.shutdown()?;
        Ok(())
    }
    #[test]
    fn run_sends_to_returns_the_resolution_error() -> Test {
        let root = Temp::new()?;
        let logger = logger(&root)?;
        let mut options = options(&root, "http://127.0.0.1:1/v1");
        options.environment.clear();
        options.model = "haiku".into();
        let mut run = run(options, &logger)?;
        run.keyring = Box::new(|_| Ok(String::new()));
        let failure = run
            .sends_to(TYPESAFE)
            .err()
            .ok_or("missing key returned a boolean")?;
        assert_eq!(result(Err(failure))["error"]["id"], "secrets.key_missing");
        logger.shutdown()?;
        Ok(())
    }

    #[test]
    fn host_deadlines_only_extend_and_waits_recheck_extensions() -> Test {
        let root = Temp::new()?;
        let options = options(&root, "http://127.0.0.1:1/v1");
        let (provider, model) = model_with("jev", &BTreeMap::new())?;
        let holdoffs = Holdoffs::default();
        let waits = Mutex::new(Vec::new());
        let wait = |delay| {
            let mut waits = waits.lock().map_err(|e| error("http.network", e))?;
            waits.push(delay);
            if waits.len() == 1 {
                holdoffs
                    .deadlines
                    .try_lock()
                    .map_err(|e| error("http.network", e))?
                    .insert("one".into(), Duration::MAX);
            }
            Ok(())
        };
        let route = Route {
            options: &options,
            provider,
            model,
            base: String::new(),
            cancel: &Cancel::default(),
            wait: &wait,
            keyring: &keyring,
            sends: &Cell::new(0),
            holdoffs: &holdoffs,
        };
        // Controlled arithmetic inputs exercise the boundary without a clock override.
        let before = Duration::MAX - Duration::from_secs(10);
        assert_eq!(
            route.defer("one", before, Duration::from_secs(5))?,
            before + Duration::from_secs(5)
        );
        assert_eq!(
            route.defer("one", before, Duration::from_secs(2))?,
            before + Duration::from_secs(5)
        );
        assert_eq!(
            route.defer("two", before, Duration::from_secs(8))?,
            before + Duration::from_secs(8)
        );
        let mut elapsed = before;
        route.holdoff("one", &mut elapsed)?;
        assert_eq!(elapsed, Duration::MAX);
        assert_eq!(
            *waits.lock().map_err(|_| "waits poisoned")?,
            vec![Duration::from_secs(5); 2]
        );
        assert_eq!(
            route.defer("two", Duration::from_secs(1), Duration::MAX)?,
            Duration::MAX
        );
        assert_eq!(
            route.defer("two", Duration::ZERO, Duration::ZERO)?,
            Duration::MAX
        );
        Ok(())
    }

    #[test]
    fn cancelled_operations_do_not_wait_for_a_locked_mutex() -> Test {
        let cancel = Arc::new(Cancel::default());
        let mutex = Arc::new(Mutex::new(()));
        let _held = mutex.lock().map_err(|_| "mutex poisoned")?;
        cancel.cancel();
        let (sender, receiver) = std::sync::mpsc::channel();
        let (worker_cancel, worker_mutex) = (Arc::clone(&cancel), Arc::clone(&mutex));
        std::thread::spawn(move || {
            let locked = worker_cancel.lock(&worker_mutex).map(|_| Value::Null);
            let _ = sender.send(result(locked));
            let waited = worker_cancel.wait(Duration::MAX).map(|()| Value::Null);
            let _ = sender.send(result(waited));
        });
        // The deadlines are failure-only backstops: both calls must return at once.
        let locked = receiver
            .recv_timeout(Duration::from_secs(20))
            .map_err(|e| format!("cancel.lock did not return: {e}"))?;
        assert_eq!(locked["error"]["id"], "decision.cancelled");
        let waited = receiver
            .recv_timeout(Duration::from_secs(20))
            .map_err(|e| format!("cancel.wait(Duration::MAX) did not return: {e}"))?;
        assert_eq!(waited["ok"], true, "{waited}");
        assert!(cancel.is_cancelled());
        Ok(())
    }

    #[test]
    fn cancel_signals_a_waiter_on_the_condition() -> Test {
        let cancel = Cancel::default();
        let state = cancel.state.lock().map_err(|_| "cancel poisoned")?;
        let expired = std::thread::scope(|scope| {
            // The canceller cannot take the lock until wait_timeout releases it.
            scope.spawn(|| cancel.cancel());
            // The deadline is a failure-only watchdog: cancel() must signal first.
            cancel
                .changed
                .wait_timeout(state, Duration::from_secs(20))
                .map(|(_, watchdog)| watchdog.timed_out())
                .map_err(|_| "cancel poisoned")
        })?;
        assert!(!expired, "cancel() did not signal the waiter");
        Ok(())
    }

    #[test]
    fn timeout_is_per_call_and_sets_the_transport_configuration() -> Test {
        let root = Temp::new()?;
        let cancel = Cancel::default();
        let sends = Cell::new(0);
        for timeout in [
            Duration::from_secs(120),
            Duration::from_millis(50),
            Duration::from_secs(120),
        ] {
            let mut options = options(&root, "http://127.0.0.1:1/v1");
            options.timeout = timeout;
            let (provider, model) = model_with("jev", &BTreeMap::new())?;
            let route = Route {
                options: &options,
                provider,
                model,
                base: String::new(),
                cancel: &cancel,
                wait: &no_wait,
                keyring: &keyring,
                sends: &sends,
                holdoffs: &Holdoffs::default(),
            };
            assert_eq!(route.agent().config().timeouts().global, Some(timeout));
        }
        Ok(())
    }

    #[test]
    fn cancellation_keeps_auth_errors_and_overrides_only_success() -> Test {
        let root = Temp::new()?;
        let logger = logger(&root)?;
        let body = json!({"error":"controlled remote refusal"});
        let stub = Stub::new(vec![http(401, body.clone())])?;
        let run = run(options(&root, &stub.url), &logger)?;
        let cancel = run.cancel.clone();
        let lower = run.send(&request()).err().ok_or("401 accepted")?;
        assert!(!cancel.is_cancelled());
        assert_eq!(stub.finish()?.len(), 1);
        cancel.cancel();
        let failure = result(after_cancel(Err(lower), &cancel));
        assert_eq!(failure["error"]["id"], "http.auth");
        assert_eq!(failure["error"]["message"], body.to_string());
        assert!(cancel.is_cancelled());
        logger.shutdown()?;
        assert_eq!(
            result(after_cancel(Ok(json!(null)), &cancel))["error"]["id"],
            "decision.cancelled"
        );
        Ok(())
    }

    #[test]
    fn hostname_command_failure_is_a_typed_config_error() -> Test {
        let failure = super::hostname(Err(std::io::Error::other("controlled hostname failure")))
            .err()
            .ok_or("hostname failure accepted")?;
        let value = result(Err(failure));
        assert_eq!(value["error"]["id"], "config.repo");
        assert!(
            value["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("controlled hostname failure"))
        );
        Ok(())
    }

    #[test]
    fn shutdown_failure_keeps_attempts_and_session_id() -> Test {
        let mut original = error("http.auth", "provider refused the request");
        let Error::Failure {
            attempts,
            session_id,
            ..
        } = &mut original;
        *attempts = Some(1);
        *session_id = Some("session-1".into());
        let value = after_failure(
            result(Err(original)),
            Err("controlled shutdown failure"),
            "log.shutdown",
        );
        assert_eq!(value["error"]["id"], "http.auth");
        assert_eq!(value["error"]["attempts"], 1);
        assert_eq!(value["error"]["session_id"], "session-1");
        assert!(value["error"]["message"].as_str().is_some_and(|message| {
            message.contains("log.shutdown")
                && message.contains("provider refused the request")
                && message.contains("controlled shutdown failure")
        }));
        Ok(())
    }
}
