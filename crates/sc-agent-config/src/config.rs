//! Configuration types, bounds and [`load`].

use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::marker::PhantomData;
use std::net::IpAddr;
use std::num::{NonZeroU16, NonZeroU32, NonZeroUsize};
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::time::Duration;

use sc_agent_types::{Catalog, ModelId, ReleaseDate, ShortName, VarName};
use sc_config::{AppName, ConfigError, Loader};
use serde::de::{Error as _, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_path_to_error::Segment;

use crate::error::AgentConfigError;

/// The sc-config application name; overrides use the prefix `SC_AGENT__`.
pub const APP_NAME: &str = "sc-agent";

/// Workers when `workers` is absent: 8 (REQ-SAG-0701).
pub const DEFAULT_WORKERS: NonZeroUsize = NonZeroUsize::MIN.saturating_add(7);

/// Sends per request when `max_sends` is absent: 5 (REQ-SAG-0601).
pub const DEFAULT_MAX_SENDS: NonZeroU32 = NonZeroU32::MIN.saturating_add(4);

/// Most workers a config may ask for: 64 (REQ-SAG-0013).
///
/// One OS thread runs per worker, so an unbounded value could exhaust threads
/// at start-up.
pub const MAX_WORKERS: usize = 64;

/// Most sends per request a config may ask for: 10 (REQ-SAG-0013).
///
/// The 90 s resend budget makes more sends pointless.
pub const MAX_MAX_SENDS: u32 = 10;

/// Most `models` entries a config may hold: 64 (REQ-SAG-0013).
pub const MAX_MODELS: usize = 64;

/// Most `local_runners` entries a config may hold: 64 (REQ-SAG-0013).
pub const MAX_LOCAL_RUNNERS: usize = 64;

/// Largest `default.json` or `local.json` read: 1 MiB.
///
/// sc-config 0.2.0 reads each file whole (`std::fs::read`, no limit), so
/// [`load`] checks the size first. A real config is a few kilobytes.
pub const MAX_CONFIG_FILE_BYTES: u64 = 1024 * 1024;

/// A local runner's request timeout when `request_timeout_secs` is absent (REQ-SAG-0015).
pub const DEFAULT_RUNNER_REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

/// A local runner's model-load timeout when `load_timeout_secs` is absent (REQ-SAG-0015).
pub const DEFAULT_RUNNER_LOAD_TIMEOUT: Duration = Duration::from_secs(600);

/// Allowed `request_timeout_secs` values (REQ-SAG-0015).
pub const RUNNER_REQUEST_TIMEOUT_SECS: RangeInclusive<u32> = 10..=3600;

/// Allowed `load_timeout_secs` values (REQ-SAG-0015).
pub const RUNNER_LOAD_TIMEOUT_SECS: RangeInclusive<u32> = 10..=7200;

/// The global configuration (`~/.sc-agent/default.json`).
///
/// [`Default`] has no models, no `log_root`, 8 workers, 5 sends and no
/// runners. Unknown keys are rejected.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct AgentConfig {
    /// Extra catalog rows keyed by model id, at most [`MAX_MODELS`] (REQ-SAG-0102).
    #[serde(default, deserialize_with = "models_in_bounds")]
    pub models: BTreeMap<ModelId, ExtraModel>,
    /// The log root; absent means [`default_dir`] (REQ-SAG-0003, REQ-SAG-0005).
    #[serde(default)]
    pub log_root: Option<PathBuf>,
    /// Batch workers, 1..=[`MAX_WORKERS`] (REQ-SAG-0701, REQ-SAG-0013).
    #[serde(default = "default_workers", deserialize_with = "workers_in_bounds")]
    pub workers: NonZeroUsize,
    /// Sends per request for every provider, 1..=[`MAX_MAX_SENDS`] (REQ-SAG-0601, REQ-SAG-0013).
    #[serde(
        default = "default_max_sends",
        deserialize_with = "max_sends_in_bounds"
    )]
    pub max_sends: NonZeroU32,
    /// Local decision runners keyed by runner name, at most [`MAX_LOCAL_RUNNERS`] (REQ-SAG-0801).
    #[serde(default, deserialize_with = "runners_in_bounds")]
    pub local_runners: BTreeMap<RunnerName, LocalRunner>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            models: BTreeMap::new(),
            log_root: None,
            workers: DEFAULT_WORKERS,
            max_sends: DEFAULT_MAX_SENDS,
            local_runners: BTreeMap::new(),
        }
    }
}

impl AgentConfig {
    /// Returns `log_root`, else [`default_dir`] (REQ-SAG-0003).
    ///
    /// This is the `<log_root>` of the ADR-SAG-0002 session-log layout.
    ///
    /// # Errors
    ///
    /// [`AgentConfigError::HomeUnknown`] when `log_root` is absent and there
    /// is no home directory.
    pub fn log_root(&self) -> Result<PathBuf, AgentConfigError> {
        match &self.log_root {
            Some(root) => Ok(root.clone()),
            None => default_dir(),
        }
    }

    /// Returns the built-in catalog plus every `models` entry.
    ///
    /// # Errors
    ///
    /// [`AgentConfigError::Catalog`] when a configured model id already
    /// belongs to another line.
    pub fn catalog(&self) -> Result<Catalog, AgentConfigError> {
        self.models
            .iter()
            .try_fold(Catalog::builtin(), |catalog, (model, extra)| {
                catalog.with_config_model(model.clone(), extra.line, extra.released)
            })
            .map_err(AgentConfigError::from)
    }
}

/// One extra catalog row: the model's line and release date.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ExtraModel {
    /// The short name whose line the model joins.
    pub line: ShortName,
    /// The release date; the newest model of a line is its latest.
    pub released: ReleaseDate,
}

/// Why [`RunnerName::try_from`] refuses a built-in short name; [`load`] maps it to `config.runner_name`.
const RUNNER_NAME_IS_SHORT_NAME: &str = "runner name must not be a built-in short name";

/// A local runner's name: the `local_runners` key (REQ-SAG-0801).
///
/// Non-empty and not a built-in short name (ASCII case-insensitive), so
/// `decide` can tell a runner from a model line by name. A short name is
/// refused while the config is deserialized; [`load`] reports it as
/// [`AgentConfigError::RunnerName`] with the key path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct RunnerName(String);

impl RunnerName {
    /// Returns the name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RunnerName {
    type Error = &'static str;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        if text.is_empty() {
            return Err("runner name must not be empty");
        }
        if ShortName::ALL
            .iter()
            .any(|short| short.as_str().eq_ignore_ascii_case(&text))
        {
            return Err(RUNNER_NAME_IS_SHORT_NAME);
        }
        Ok(Self(text))
    }
}

impl AsRef<str> for RunnerName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Derived `Ord`/`Eq` on the inner `String` agree with `str`, so maps can be searched by `&str`.
impl Borrow<str> for RunnerName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RunnerName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// One local decision runner (REQ-SAG-0801); its name is the map key.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct LocalRunner {
    /// The daemon kind.
    pub kind: RunnerKind,
    /// The host the daemon listens on.
    pub host: RunnerHost,
    /// The port the daemon listens on; 0 is rejected (REQ-SAG-0013).
    pub port: NonZeroU16,
    /// The model the runner serves.
    pub model: ModelId,
    /// The variable holding the runner's key, resolved with `sc_agent_secrets::resolve_var`.
    ///
    /// Local runners are plain http, so [`load`] refuses `key_env` unless
    /// [`RunnerHost::is_loopback`] (REQ-SAG-0013).
    #[serde(default)]
    pub key_env: Option<VarName>,
    /// Request timeout in seconds, within [`RUNNER_REQUEST_TIMEOUT_SECS`] (REQ-SAG-0015).
    #[serde(default, deserialize_with = "request_timeout_in_bounds")]
    pub request_timeout_secs: Option<NonZeroU32>,
    /// Model-load timeout in seconds, within [`RUNNER_LOAD_TIMEOUT_SECS`] (REQ-SAG-0015).
    #[serde(default, deserialize_with = "load_timeout_in_bounds")]
    pub load_timeout_secs: Option<NonZeroU32>,
}

impl LocalRunner {
    /// Returns `request_timeout_secs`, else [`DEFAULT_RUNNER_REQUEST_TIMEOUT`] (300 s).
    #[must_use]
    pub fn request_timeout(&self) -> Duration {
        self.request_timeout_secs
            .map_or(DEFAULT_RUNNER_REQUEST_TIMEOUT, secs)
    }

    /// Returns `load_timeout_secs`, else [`DEFAULT_RUNNER_LOAD_TIMEOUT`] (600 s).
    #[must_use]
    pub fn load_timeout(&self) -> Duration {
        self.load_timeout_secs
            .map_or(DEFAULT_RUNNER_LOAD_TIMEOUT, secs)
    }
}

fn secs(value: NonZeroU32) -> Duration {
    Duration::from_secs(u64::from(value.get()))
}

/// The closed set of local daemon kinds (REQ-SAG-0013).
///
/// sc-decision-local maps each to its own daemon kind; a new daemon adds a
/// variant here and there, so both crates match exhaustively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunnerKind {
    /// A Strands daemon.
    Strands,
    /// A Laya daemon.
    Laya,
    /// A Mapika daemon.
    Mapika,
    /// Any other daemon speaking the generic protocol.
    Generic,
}

/// A runner host: a hostname or an IP address.
///
/// A hostname is 1..=253 characters of dot-separated labels, each 1..=63
/// characters of `[A-Za-z0-9-]` with no leading or trailing `-`. Paths, `@`,
/// `:` ports, whitespace and empty text are rejected.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct RunnerHost(String);

impl RunnerHost {
    /// Returns the host text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether the host is 127.0.0.0/8, `::1` or `localhost` (ASCII case-insensitive).
    #[must_use]
    pub fn is_loopback(&self) -> bool {
        match self.0.parse::<IpAddr>() {
            Ok(ip) => ip.is_loopback(),
            Err(_) => self.0.eq_ignore_ascii_case("localhost"),
        }
    }
}

impl TryFrom<String> for RunnerHost {
    type Error = &'static str;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        if text.parse::<IpAddr>().is_ok() || is_hostname(&text) {
            Ok(Self(text))
        } else {
            Err("host must be a hostname or IP address")
        }
    }
}

impl AsRef<str> for RunnerHost {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

fn is_hostname(text: &str) -> bool {
    (1..=253).contains(&text.len())
        && text.split('.').all(|label| {
            (1..=63).contains(&label.len())
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                && !label.starts_with('-')
                && !label.ends_with('-')
        })
}

fn models_in_bounds<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<ModelId, ExtraModel>, D::Error> {
    deserializer.deserialize_map(BoundedMap::new("models", MAX_MODELS))
}

fn runners_in_bounds<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<RunnerName, LocalRunner>, D::Error> {
    deserializer.deserialize_map(BoundedMap::new("local_runners", MAX_LOCAL_RUNNERS))
}

/// Reads a JSON object into a map, refusing it before the entry after `limit`.
struct BoundedMap<K, V> {
    key: &'static str,
    limit: usize,
    entries: PhantomData<fn() -> (K, V)>,
}

impl<K, V> BoundedMap<K, V> {
    const fn new(key: &'static str, limit: usize) -> Self {
        Self {
            key,
            limit,
            entries: PhantomData,
        }
    }
}

impl<'de, K: Deserialize<'de> + Ord, V: Deserialize<'de>> Visitor<'de> for BoundedMap<K, V> {
    type Value = BTreeMap<K, V>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "an object of at most {} entries", self.limit)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
        let mut map = BTreeMap::new();
        while let Some(key) = access.next_key::<K>()? {
            if map.len() >= self.limit {
                return Err(A::Error::custom(format!(
                    "{} must have at most {} entries",
                    self.key, self.limit
                )));
            }
            let value = access.next_value::<V>()?;
            map.insert(key, value);
        }
        Ok(map)
    }
}

const fn default_workers() -> NonZeroUsize {
    DEFAULT_WORKERS
}

const fn default_max_sends() -> NonZeroU32 {
    DEFAULT_MAX_SENDS
}

fn workers_in_bounds<'de, D: Deserializer<'de>>(deserializer: D) -> Result<NonZeroUsize, D::Error> {
    let value = usize::deserialize(deserializer)?;
    NonZeroUsize::new(value)
        .filter(|workers| workers.get() <= MAX_WORKERS)
        .ok_or_else(|| D::Error::custom(format!("workers must be 1..={MAX_WORKERS}, got {value}")))
}

fn max_sends_in_bounds<'de, D: Deserializer<'de>>(deserializer: D) -> Result<NonZeroU32, D::Error> {
    let value = u32::deserialize(deserializer)?;
    NonZeroU32::new(value)
        .filter(|sends| sends.get() <= MAX_MAX_SENDS)
        .ok_or_else(|| {
            D::Error::custom(format!(
                "max_sends must be 1..={MAX_MAX_SENDS}, got {value}"
            ))
        })
}

fn secs_in_bounds<'de, D: Deserializer<'de>>(
    deserializer: D,
    key: &str,
    bounds: &RangeInclusive<u32>,
) -> Result<Option<NonZeroU32>, D::Error> {
    let Some(value) = Option::<u32>::deserialize(deserializer)? else {
        return Ok(None);
    };
    NonZeroU32::new(value)
        .filter(|_| bounds.contains(&value))
        .map(Some)
        .ok_or_else(|| {
            D::Error::custom(format!(
                "{key} must be {}..={}, got {value}",
                bounds.start(),
                bounds.end()
            ))
        })
}

fn request_timeout_in_bounds<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<NonZeroU32>, D::Error> {
    secs_in_bounds(
        deserializer,
        "request_timeout_secs",
        &RUNNER_REQUEST_TIMEOUT_SECS,
    )
}

fn load_timeout_in_bounds<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<NonZeroU32>, D::Error> {
    secs_in_bounds(deserializer, "load_timeout_secs", &RUNNER_LOAD_TIMEOUT_SECS)
}

/// Returns `~/.sc-agent`, the config directory and default log root (REQ-SAG-0003).
///
/// The home directory joined with `.sc-agent` on every platform; the home
/// directory comes from [`std::env::home_dir`]. Every log is written under
/// `<log_root>/logs/`: general logs at `logs/<cli>.log.jsonl`, session logs at
/// `logs/<repo>/<agent_role>/<project>/`.
///
/// # Errors
///
/// [`AgentConfigError::HomeUnknown`] when there is no (non-empty) home directory.
pub fn default_dir() -> Result<PathBuf, AgentConfigError> {
    std::env::home_dir()
        .filter(|home| !home.as_os_str().is_empty())
        .map(|home| home.join(".sc-agent"))
        .ok_or(AgentConfigError::HomeUnknown)
}

/// Loads `<config_dir>/default.json`, optional `local.json` and `SC_AGENT__*` overrides.
///
/// `environment` is the only override source (decide passes the process
/// environment and [`default_dir`]; tests pass their own). Values are checked
/// against their types and bounds with the key path of any failure.
///
/// With no `default.json` the result is [`AgentConfig::default`], unless an
/// `SC_AGENT__` variable is set or a `local.json` exists: then the missing
/// file is an error, so an override is never silently ignored (the
/// `local.json` case is an accepted decision, bead agt-4p2.4 notes).
///
/// # Errors
///
/// - [`AgentConfigError::NotAFile`] when `default.json` or `local.json`
///   exists but is not a regular file (a FIFO, a device, a directory),
///   checked from its metadata before any read.
/// - [`AgentConfigError::FileTooLarge`] when `default.json` or `local.json`
///   is larger than [`MAX_CONFIG_FILE_BYTES`] (checked before reading).
/// - [`AgentConfigError::Load`] when sc-config fails (unreadable file, bad
///   JSON, an override it cannot apply, or a missing `default.json` with an
///   override present).
/// - [`AgentConfigError::Invalid`] when a value fails its type or bound,
///   including unknown keys and more than [`MAX_MODELS`] models or
///   [`MAX_LOCAL_RUNNERS`] runners; `path` names the key, such as `workers`
///   or `local_runners.laya.host`.
/// - [`AgentConfigError::RunnerName`] when a runner is named like a short name.
/// - [`AgentConfigError::KeyOnPlainHttp`] when a runner with `key_env` has a
///   host that is not loopback.
pub fn load(
    config_dir: PathBuf,
    environment: Vec<(OsString, OsString)>,
) -> Result<AgentConfig, AgentConfigError> {
    let app = AppName::new(APP_NAME)?;
    let prefix = app.env_prefix();
    let has_override = environment
        .iter()
        .any(|(name, _)| name.as_encoded_bytes().starts_with(prefix.as_bytes()));
    // An unreadable directory counts as "local.json may exist": the error is kept.
    let has_local = config_dir.join("local.json").try_exists().unwrap_or(true);
    for file in ["default.json", "local.json"] {
        check_file(&config_dir.join(file))?;
    }
    let value = match Loader::new(app, config_dir, environment).load::<serde_json::Value>() {
        Ok(value) => value,
        Err(ConfigError::MissingDefault { .. }) if !has_override && !has_local => {
            return Ok(AgentConfig::default());
        }
        Err(error) => return Err(error.into()),
    };
    let config: AgentConfig = serde_path_to_error::deserialize(value).map_err(invalid)?;
    check_runners(&config)?;
    Ok(config)
}

/// Refuses, from its metadata alone, a config file that is not a regular file (a FIFO or
/// device, whose read can block forever) or is over [`MAX_CONFIG_FILE_BYTES`]; a missing or
/// unreadable one is sc-config's to report.
fn check_file(path: &std::path::Path) -> Result<(), AgentConfigError> {
    match std::fs::metadata(path) {
        Ok(metadata) if !metadata.is_file() => Err(AgentConfigError::NotAFile {
            file: path.to_path_buf(),
        }),
        Ok(metadata) if metadata.len() > MAX_CONFIG_FILE_BYTES => {
            Err(AgentConfigError::FileTooLarge {
                file: path.to_path_buf(),
                bytes: metadata.len(),
            })
        }
        Ok(_) | Err(_) => Ok(()),
    }
}

/// Maps a deserialization failure to `config.invalid`, or `config.runner_name` for a short-name runner key.
fn invalid(error: serde_path_to_error::Error<serde_json::Error>) -> AgentConfigError {
    let path = error.path().to_string();
    let message = error.inner().to_string();
    if message == RUNNER_NAME_IS_SHORT_NAME {
        let segments: Vec<&Segment> = error.path().iter().collect();
        if let [Segment::Map { key: field }, Segment::Map { key: name }] = segments.as_slice()
            && field == "local_runners"
        {
            return AgentConfigError::RunnerName {
                name: name.clone(),
                path,
            };
        }
    }
    AgentConfigError::Invalid { path, message }
}

/// The runner check serde cannot express: no key over plain http off the machine.
fn check_runners(config: &AgentConfig) -> Result<(), AgentConfigError> {
    for (name, runner) in &config.local_runners {
        if runner.key_env.is_some() && !runner.host.is_loopback() {
            return Err(AgentConfigError::KeyOnPlainHttp {
                path: format!("local_runners.{name}.key_env"),
                host: runner.host.as_str().to_owned(),
            });
        }
    }
    Ok(())
}
