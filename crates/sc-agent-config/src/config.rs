//! Configuration types and validation bounds.

use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;
use std::net::IpAddr;
use std::num::{NonZeroU16, NonZeroU32, NonZeroUsize};
use std::ops::RangeInclusive;
use std::time::Duration;

use sc_agent_types::{Catalog, ModelId, ReleaseDate, ShortName, VarName};
use serde::de::{Error as _, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_path_to_error::Segment;

use crate::error::AgentConfigError;

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

/// Largest configuration layer read: 1 MiB ([`crate::layers::MAX_FILE_BYTES`]).
pub const MAX_CONFIG_FILE_BYTES: u64 = crate::layers::MAX_FILE_BYTES;

/// A local runner's request timeout when `request_timeout_secs` is absent (REQ-SAG-0015).
pub const DEFAULT_RUNNER_REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

/// A local runner's model-load timeout when `load_timeout_secs` is absent (REQ-SAG-0015).
pub const DEFAULT_RUNNER_LOAD_TIMEOUT: Duration = Duration::from_secs(600);

/// Allowed `request_timeout_secs` values (REQ-SAG-0015).
pub const RUNNER_REQUEST_TIMEOUT_SECS: RangeInclusive<u32> = 10..=3600;

/// Allowed `load_timeout_secs` values (REQ-SAG-0015).
pub const RUNNER_LOAD_TIMEOUT_SECS: RangeInclusive<u32> = 10..=7200;

/// The validated values resolved from the CLI configuration layers.
///
/// [`Default`] has no models, 8 workers, 5 sends and no
/// runners. Unknown keys are rejected.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct AgentConfig {
    #[serde(skip_deserializing)]
    // Validated by sc_agent_types::is_valid_slug before storage by load_layered.
    pub(crate) project: Option<String>,
    #[serde(skip)]
    pub(crate) cli: crate::CliName,
    /// Extra catalog rows keyed by model id, at most [`MAX_MODELS`] (REQ-SAG-0102).
    #[serde(default, deserialize_with = "models_in_bounds")]
    pub models: BTreeMap<ModelId, ExtraModel>,
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
            project: None,
            cli: crate::CliName::Decidr,
            models: BTreeMap::new(),
            workers: DEFAULT_WORKERS,
            max_sends: DEFAULT_MAX_SENDS,
            local_runners: BTreeMap::new(),
        }
    }
}

impl AgentConfig {
    /// The resolved path-safe project/phase slug, with no built-in default.
    ///
    /// Validation uses the same sc-agent-types rule as the session logger.
    #[must_use]
    pub fn project(&self) -> Option<&str> {
        self.project.as_deref()
    }

    /// Requires a project/phase before a session is sent or written.
    ///
    /// # Errors
    /// Returns [`AgentConfigError::ProjectRequired`] naming every setting place.
    pub fn require_project(&self) -> Result<&str, AgentConfigError> {
        self.project().ok_or(AgentConfigError::ProjectRequired {
            cli: self.cli.file_name(),
        })
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
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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
/// `decidr` can tell a runner from a model line by name. A short name is
/// refused while the config is deserialized; [`load`] reports it as
/// [`AgentConfigError::RunnerName`] with the key path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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
    #[serde(
        default,
        deserialize_with = "request_timeout_in_bounds",
        serialize_with = "secs_or::<300, _>"
    )]
    pub request_timeout_secs: Option<NonZeroU32>,
    /// Model-load timeout in seconds, within [`RUNNER_LOAD_TIMEOUT_SECS`] (REQ-SAG-0015).
    #[serde(
        default,
        deserialize_with = "load_timeout_in_bounds",
        serialize_with = "secs_or::<600, _>"
    )]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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

// Doctor shows an unset timeout as its effective default (DEFAULT_RUNNER_*_TIMEOUT).
fn secs_or<const DEFAULT: u32, S: Serializer>(
    secs: &Option<NonZeroU32>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_u32(secs.map_or(DEFAULT, NonZeroU32::get))
}

const _: () = assert!(DEFAULT_RUNNER_REQUEST_TIMEOUT.as_secs() == 300);
const _: () = assert!(DEFAULT_RUNNER_LOAD_TIMEOUT.as_secs() == 600);

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

/// Maps a deserialization failure to `config.invalid`, or `config.runner_name` for a short-name runner key.
pub(crate) fn invalid(error: serde_path_to_error::Error<serde_json::Error>) -> AgentConfigError {
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
pub(crate) fn check_runners(config: &AgentConfig) -> Result<(), AgentConfigError> {
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
