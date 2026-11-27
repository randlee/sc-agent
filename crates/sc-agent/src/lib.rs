use sc_config::{
    AppName, ConfigError, EnvSource, FileSource, Layer, LayerSource, Resolved, Slot, SourceKind,
    StandardContext, StandardLayers,
};
use sc_observability::{LoggerConfig, ServiceName, v2::Logger};
use serde::{Deserialize, Serialize};
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
                    #[cfg(test)]
                    route_tests::LOCK_WAIT.with(|hook| {
                        if let Some(waiting) = hook.borrow_mut().take() {
                            waiting();
                        }
                    });
                    std::thread::park_timeout(Duration::from_millis(20))
                }
                Err(TryLockError::Poisoned(e)) => return Err(error("http.network", e)),
            }
        }
    }
    pub fn wait(&self, delay: Duration) {
        if let Ok(state) = self.state.lock() {
            #[cfg(test)]
            route_tests::WAIT_ENTERED.with(|hook| {
                if let Some(waiting) = hook.borrow_mut().take() {
                    waiting();
                }
            });
            let _ = self
                .changed
                .wait_timeout_while(state, delay, |state| !*state);
        }
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
pub fn error(id: &'static str, message: impl ToString) -> Error {
    let category = match id {
        "secrets.key_missing"
        | "secrets.key_malformed"
        | "secrets.keyring_read"
        | "secrets.keyring_timeout"
        | "http.auth" => "auth",
        "http.network" => "network",
        "decision.api_status" => "other",
        "http.provider_busy" => "busy",
        "http.quota_exhausted" => "quota",
        "decision.cancelled" => "cancelled",
        "decision.response_parse" | "decision.schema_mismatch" => "response_invalid",
        s if s.starts_with("log.") => "log",
        s if s.starts_with("config.") => "config",
        _ => "validation",
    };
    Error::Failure {
        id,
        message: message.to_string(),
        category,
        action: match category {
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
            _ => "Fix the named input or provider response.",
        },
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

fn deserialize_model_release<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let date = String::deserialize(deserializer)?;
    {
        let mut parts = date.split('-');
        let year_part = parts.next().unwrap_or_default();
        let month_part = parts.next().unwrap_or_default();
        let day_part = parts.next().unwrap_or_default();
        let valid_shape = year_part.len() == 4
            && month_part.len() == 2
            && day_part.len() == 2
            && parts.next().is_none()
            && [year_part, month_part, day_part]
                .iter()
                .all(|part| part.bytes().all(|byte| byte.is_ascii_digit()));
        let valid_date = if valid_shape {
            let year = year_part.parse::<u32>().unwrap_or(0);
            let month = month_part.parse::<u32>().unwrap_or(0);
            let day = day_part.parse::<u32>().unwrap_or(0);
            let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
            let days = match month {
                1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                4 | 6 | 9 | 11 => 30,
                2 if leap => 29,
                2 => 28,
                _ => 0,
            };
            day > 0 && day <= days
        } else {
            false
        };
        if !valid_date {
            return Err(serde::de::Error::custom(
                "released must be a YYYY-MM-DD date",
            ));
        }
    }
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
    if selected.is_none()
        && let Some((head, _)) = path.split_once('.')
        && PROVIDERS.iter().any(|p| p.url.is_empty() && p.name == head)
    {
        return Err(invalid_setting(resolved, head, missing_setting(path)));
    }
    serde_json::from_value(selected.cloned().unwrap_or(default))
        .map_err(|e| invalid_setting(resolved, path, e))
}
impl Settings {
    pub fn project_required(&self) -> Result<&str, Error> {
        self.project.as_deref().filter(|s| !s.is_empty()).ok_or_else(||error("config.missing_project", "Set project/phase through --project, SC_PROJECT, local decidr.local.json, local decidr.json, local sc-agent.json, global decidr.local.json, global decidr.json, or global sc-agent.json"))
    }
}
const KEYS: &[&str] = &[
    "project",
    "workers",
    "models",
    "laya.host",
    "laya.port",
    "decider_4b.host",
    "decider_4b.port",
    "decider_2b.host",
    "decider_2b.port",
    "max_sends",
    "decidr.luna-system-prompt",
    "decidr.haiku-system-prompt",
];
pub struct Configuration {
    pub config: Settings,
    pub rows: Vec<Value>,
    pub files: Vec<Value>,
}
struct ConfigEnv(Vec<(OsString, OsString)>);
impl LayerSource for ConfigEnv {
    fn load(&self, base: &Value) -> Result<Option<Layer>, ConfigError> {
        let variables = self
            .0
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
pub fn config(
    local: Option<&Path>,
    global: Option<&Path>,
    environment: &[(OsString, OsString)],
    project: Option<&str>,
) -> Result<Configuration, Error> {
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
            for name in ["sc-agent.json", "decidr.json", "decidr.local.json"] {
                let path = folder.join(name);
                files.push(json!({"path":path,"exists":path.exists()}));
                layers = layers.insert(
                    slot,
                    FileSource::new(path)
                        .optional()
                        .accepts(KEYS.iter().map(|key| key.split('.').collect::<Vec<_>>())),
                );
            }
        }
    }
    let flags = project
        .map(|project| json!({"project":project}))
        .unwrap_or(json!({}));
    let resolved = layers
        .insert(Slot::Env, ConfigEnv(environment.to_vec()))
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
            let host = setting(&resolved, &format!("{name}.host"), Value::Null)?;
            let port = setting(&resolved, &format!("{name}.port"), Value::Null)?;
            local.insert(name.into(), Runner { host, port });
        }
    }
    let config = Settings {
        project: setting(&resolved, "project", Value::Null)?,
        workers: setting(&resolved, "workers", json!(8))?,
        models: setting(&resolved, "models", json!({}))?,
        local,
        max_sends: setting(&resolved, "max_sends", json!(5))?,
        decidr: setting(&resolved, "decidr", json!({}))?,
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
    let rows = KEYS
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
    std::env::var_os(format!(
        "{}_LOG_ROOT",
        name.to_uppercase().replace('-', "_")
    ))
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
                KeyringError::Read(_) => error(
                    "secrets.keyring_read",
                    format!("The platform keyring is unavailable for {variable}. Set {variable} in the environment or repository .env."),
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
            },
            model: String::new(),
            provider: None,
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
        assert!(!message.contains("controlled refusal"));
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
            assert!(!message.contains("denied"));
            assert!(!message.contains("missing service"));
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
pub static PROVIDERS: &[Provider] = &[
    Provider {
        name: "typesafe",
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
/// Resolve a model name using only the built-in tables.
pub fn model(name: &str, provider: Option<&str>) -> Result<(&'static Provider, String), Error> {
    model_with(name, provider, &BTreeMap::new())
}
/// Resolve a model name to its first candidate; a configured entry for `name`
/// wins over the built-in tables unless an explicit `provider` names a different one.
pub fn model_with(
    name: &str,
    provider: Option<&str>,
    configured: &BTreeMap<String, ModelEntry>,
) -> Result<(&'static Provider, String), Error> {
    Ok(candidates(name, provider, configured)?.remove(0))
}
/// Every provider row that serves `name`, newest release first, equal dates in table order.
fn candidates(
    name: &str,
    provider: Option<&str>,
    configured: &BTreeMap<String, ModelEntry>,
) -> Result<Vec<(&'static Provider, String)>, Error> {
    if let Some(entry) = configured.get(name) {
        let p = provider_named(&entry.provider)?;
        if provider.is_none_or(|requested| requested == entry.provider) {
            return Ok(vec![(p, entry.id.clone())]);
        }
    }
    let chosen = provider.map(provider_named).transpose()?;
    let mut rows = Vec::new();
    for p in PROVIDERS
        .iter()
        .filter(|p| chosen.is_none_or(|c| c.name == p.name))
    {
        for (_, id, date) in p.models.iter().filter(|(s, i, _)| *s == name || *i == name) {
            rows.push((*date, p, (*id).to_owned()));
        }
    }
    rows.sort_by_key(|row| std::cmp::Reverse(row.0));
    let mut found = rows
        .into_iter()
        .map(|(_, p, id)| (p, id))
        .collect::<Vec<_>>();
    if let Some(p) = chosen.filter(|p| p.open && found.is_empty()) {
        found.push((p, name.into()));
    }
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
                    let Error::Failure { message, .. } = &mut failure;
                    message.push_str(&format!("\n{previous}"));
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
pub fn log(logger: &Logger, action: &str, fields: Value) -> Result<(), Error> {
    let event = serde_json::from_value(
        json!({"version":"v1","timestamp":sc_observability::Timestamp::now_utc(),"level":if action=="error" {sc_observability::Level::Error}else{sc_observability::Level::Info},"service":logger.service_name(),"target":"sc-agent","action":action,"identity":sc_observability::ProcessIdentity::default(),"fields":fields}),
    ).map_err(|e| error("log.invalid_event", e))?;
    logger.log(event).map_err(|e| error("log.write", e))
}

fn answer_schema(question: &Value) -> Value {
    let kind = question.get("type").and_then(Value::as_str).unwrap_or("");
    let names = if kind == "choice" {
        question
            .get("criteria")
            .and_then(Value::as_object)
            .map(|v| v.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default()
    } else {
        question
            .get("criteria")
            .and_then(Value::as_array)
            .map(|v| (0..v.len()).map(|i| i.to_string()).collect::<Vec<_>>())
            .unwrap_or_default()
    };
    let object = |value_type| {
        json!({
            "type":"object",
            "properties":names.iter().map(|name|(name.clone(),json!({"type":value_type}))).collect::<BTreeMap<_,_>>(),
            "required":names,
            "additionalProperties":false
        })
    };
    let mut fields = json!({"type":{"type":"string","enum":[kind]}});
    if let Some(fields) = fields.as_object_mut() {
        match kind {
            "choice" => {
                fields.insert("choice".into(), json!({"type":"string","enum":names}));
                fields.insert("confidence".into(), json!({"type":"number"}));
                fields.insert("probabilities".into(), object("number"));
            }
            "score" => {
                fields.insert("score".into(), json!({"type":"number"}));
                fields.insert("confidence".into(), json!({"type":"number"}));
                fields.insert("legend".into(), object("string"));
                fields.insert("probabilities".into(), object("number"));
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
    }
    let required = fields
        .as_object()
        .map(|v| v.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
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
pub struct Route<'a> {
    pub options: &'a Options,
    pub provider: &'static Provider,
    pub model: String,
    pub base: String,
    pub cancel: &'a Cancel,
    pub wait: &'a (dyn Fn(Duration) + Sync),
    pub keyring: &'a (dyn Fn(&str) -> Result<String, KeyringError> + Sync),
    pub sends: &'a Cell<u32>,
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
        let pause = host_holdoff(&host)?;
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
        for attempt in 1..=max {
            cancel.check()?;
            // A busy reply holds off senders to this host through its injected wait.
            drop(cancel.lock(&pause)?);
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
                        reply.body_mut().read_to_string(),
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
            if (busy || status == 0) && attempt < max {
                let _hold = (status != 0).then(|| cancel.lock(&pause)).transpose()?;
                (self.wait)(
                    delay.unwrap_or_else(|| Duration::from_secs(2u64.saturating_pow(attempt - 1))),
                );
                continue;
            }
            let id = match status {
                _ if quota => "http.quota_exhausted",
                _ if busy => "http.provider_busy",
                0 => "http.network",
                401 => "http.auth",
                _ => "decision.api_status",
            };
            return Err(error(id, text));
        }
        Err(error("http.network", "send count exhausted"))
    }
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

pub fn workers(value: i64) -> Result<usize, Error> {
    usize::try_from(value)
        .ok()
        .filter(|v| *v > 0)
        .ok_or_else(|| error("decidr.invalid_workers", "workers must be positive"))
}

fn after_cancel(outcome: Result<Value, Error>, cancel: &Cancel) -> Result<Value, Error> {
    outcome.and_then(|value| cancel.check().map(|()| value))
}
pub fn send(
    request: &Value,
    options: &Options,
    logger: &Logger,
    cancel: &Cancel,
    wait: &(dyn Fn(Duration) + Sync),
    keyring: &(dyn Fn(&str) -> Result<String, KeyringError> + Sync),
) -> Result<Value, Error> {
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
        let found = candidates(
            &options.model,
            options.provider.as_deref(),
            &options.settings.models,
        )?;
        let (provider, model) = resolve(found, options, keyring)?;
        let base = match options.settings.local.get(provider.name) {
            _ if !provider.url.is_empty() => provider.url.into(),
            Some(runner) => format!("http://{}:{}/v1", runner.host, runner.port),
            None => return Err(missing_setting(&format!("{}.host", provider.name))),
        };
        let route = Route {
            options,
            provider,
            model,
            base: options.base_url.clone().unwrap_or(base),
            cancel,
            wait,
            keyring,
            sends: &sends,
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
    let mut session_failure = None;
    let attach = |failure, context| contextual(failure, context, Some(&session_id));
    let prior = || format!("original outcome: {outcome:?}");
    if let Ok(project) = options.settings.project_required()
        && slug(project).is_ok()
        && slug(&options.agent_role).is_ok()
    {
        let row = json!({"session_id":session_id,"repo":options.repo,"agent_role":options.agent_role,"project":project,"request":request,"question_digests":digests,"result":match &outcome {Ok(v)=>json!({"ok":true,"value":v}),Err(e)=>json!({"ok":false,"error":e})}});
        let path = options
            .log_root
            .join("sessions")
            .join(&options.repo)
            .join(&options.agent_role)
            .join(project);
        let _append = SESSION_APPEND
            .lock()
            .map_err(|e| attach(error("config.session", e), prior()))?;
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
        session_failure = written
            .err()
            .map(|e| attach(error("log.session_write", e), prior()));
        if let Some(failure) = &session_failure
            && let Err(log_failure) = log(logger, "error", json!(failure))
        {
            outcome = Err(attach(
                log_failure,
                format!("original session error: {failure}"),
            ));
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
        let context = format!("original outcome: {outcome:?}; session error: {session_failure:?}");
        outcome = Err(attach(failure, context));
    }
    outcome
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
    if options.settings.workers == 0 {
        return Err(error("decidr.invalid_workers", "workers must be positive"));
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let (sender, receiver) = std::sync::mpsc::channel();
    Ok(std::thread::scope(|scope| {
        for _ in 0..options.settings.workers.min(requests.len()) {
            let sender = sender.clone();
            let next = &next;
            scope.spawn(move || {
                loop {
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    let Some(request) = requests.get(index) else {
                        break;
                    };
                    let value = report(
                        logger,
                        send(request, options, logger, cancel, wait, keyring),
                    );
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
    let host = std::process::Command::new("hostname")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| error("config.repo", "host name unavailable"))?;
    let name = repository_identity(origin.as_deref(), &host, &root)?;
    Ok((root, name))
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
    config_dir: Option<&Path>,
    project: Option<&str>,
) -> Result<(Configuration, PathBuf, String), Error> {
    let (root, repo) = repository()?;
    let local = root.join(".sc/sc-agent");
    let environment = std::env::vars_os().collect::<Vec<_>>();
    let local = root.join(".git").exists().then_some(local.as_path());
    Ok((
        config(local, config_dir, &environment, project)?,
        root,
        repo,
    ))
}
pub fn doctor(config_dir: Option<&Path>, project: Option<&str>) -> Result<Value, Error> {
    let (loaded, ..) = load(config_dir, project)?;
    Ok(json!({"settings":loaded.rows, "files":loaded.files, "log_root":log_root("decidr")?}))
}

pub fn report(logger: &Logger, outcome: Result<Value, Error>) -> Value {
    if let Err(error) = &outcome
        && let Err(failure) = log(logger, "error", json!(error))
    {
        let Error::Failure { session_id, .. } = error;
        return result(Err(contextual(
            failure,
            format!("original error: {error}"),
            session_id.as_deref(),
        )));
    }
    result(outcome)
}

#[cfg(test)]
extern crate self as sc_agent;
#[cfg(test)]
#[path = "../tests/support/mod.rs"]
pub mod test_support;
#[cfg(test)]
mod route_tests {
    use super::*;
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
            let found = candidates("haiku", None, &BTreeMap::new())?;
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

    thread_local! {
        pub(super) static LOCK_WAIT: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(None) };
        pub(super) static WAIT_ENTERED: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(None) };
    }

    #[test]
    fn cancel_ends_busy_holdoff_after_wait_is_entered() -> Test {
        let cancel = Arc::new(Cancel::default());
        let (entered, waiting) = std::sync::mpsc::channel();
        let (finished, outcome) = std::sync::mpsc::channel();
        let worker_cancel = cancel.clone();
        let worker = std::thread::spawn(move || {
            WAIT_ENTERED.with(|hook| {
                *hook.borrow_mut() = Some(Box::new(move || {
                    let _ = entered.send(());
                }))
            });
            worker_cancel.wait(Duration::from_secs(600));
            let _ = finished.send(());
        });
        waiting
            .recv_timeout(Duration::from_secs(10))
            .map_err(|e| format!("holdoff wait not entered: {e}"))?;
        std::thread::spawn(move || cancel.cancel());
        outcome
            .recv_timeout(Duration::from_secs(10))
            .map_err(|e| format!("cancel did not release holdoff: {e}"))?;
        worker.join().map_err(|_| "holdoff waiter panicked")?;
        Ok(())
    }

    #[test]
    fn cancel_ends_a_wait_on_the_host_holdoff() -> Test {
        let cancel = Arc::new(Cancel::default());
        let host = host_holdoff("cancel-wait.invalid")?;
        let same_host = host_holdoff("cancel-wait.invalid")?;
        let held = host.lock().map_err(|_| "holdoff poisoned")?;
        let (finished, outcome) = std::sync::mpsc::channel();
        let waiting_cancel = cancel.clone();
        let reached = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_reached = reached.clone();
        let work = std::thread::spawn(move || {
            let flag = waiting_cancel.clone();
            LOCK_WAIT.with(|hook| {
                *hook.borrow_mut() = Some(Box::new(move || {
                    worker_reached.store(true, Ordering::SeqCst);
                    flag.cancel();
                }));
            });
            let result = waiting_cancel.lock(&same_host).map(|_| json!(null));
            let _ = finished.send(result);
        });
        let result = outcome.recv_timeout(Duration::from_secs(10));
        drop(held);
        if matches!(result, Err(RecvTimeoutError::Timeout)) {
            return Err(format!(
                "lock waiter timed out; contention hook reached={}",
                reached.load(Ordering::SeqCst)
            )
            .into());
        }
        work.join().map_err(|_| "lock waiter panicked")?;
        assert!(cancel.is_cancelled(), "contention hook was not reached");
        assert_eq!(super::result(result?)["error"]["id"], "decision.cancelled");
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
            let (provider, model) = model("jev", None)?;
            let route = Route {
                options: &options,
                provider,
                model,
                base: String::new(),
                cancel: &cancel,
                wait: &no_wait,
                keyring: &keyring,
                sends: &sends,
            };
            assert_eq!(route.agent().config().timeouts().global, Some(timeout));
        }
        Ok(())
    }

    #[test]
    fn cancellation_keeps_auth_errors_and_overrides_only_success() -> Test {
        let cancel = Cancel::default();
        cancel.cancel();
        let failure = result(after_cancel(
            Err(error("http.auth", "remote refusal")),
            &cancel,
        ));
        assert_eq!(failure["error"]["id"], "http.auth");
        assert_eq!(failure["error"]["message"], "remote refusal");
        assert_eq!(
            result(after_cancel(Ok(json!(null)), &cancel))["error"]["id"],
            "decision.cancelled"
        );
        Ok(())
    }
}
