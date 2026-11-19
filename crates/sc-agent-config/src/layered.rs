//! Explicit-input configuration layers shared by the CLIs.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::{AgentConfig, AgentConfigError, MAX_CONFIG_FILE_BYTES};

/// Shared-scope file in each configuration folder.
pub const SHARED_FILE: &str = "sc-agent.json";
/// The project/phase variable shared by every CLI.
pub const PROJECT_ENV: &str = "SC_PROJECT";

/// The closed CLI scope table of ADR-SAG-0020.
/// Adding a CLI requires updating that table and releasing sc-agent-config.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CliName {
    /// The decision CLI.
    #[default]
    Decidr,
    /// The tree-sitter CLI.
    Sitr,
}
impl CliName {
    /// Program name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Decidr => "decidr",
            Self::Sitr => "sitr",
        }
    }
    /// Configuration environment prefix (without an extra underscore).
    #[must_use]
    pub fn env_prefix(self) -> &'static str {
        match self {
            Self::Decidr => "DECIDR_",
            Self::Sitr => "SITR_",
        }
    }
    /// The CLI-specific configuration file.
    #[must_use]
    pub fn file_name(self) -> &'static str {
        match self {
            Self::Decidr => "decidr.json",
            Self::Sitr => "sitr.json",
        }
    }
}

/// Where a resolved setting originated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// CLI flag, including its leading `--`.
    Flag(String),
    /// Environment variable name.
    Env(String),
    /// Configuration file path.
    File(PathBuf),
    /// Built-in value or an unset optional setting.
    Default,
}
impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Flag(s) | Self::Env(s) => f.write_str(s),
            Self::File(p) => p.display().fmt(f),
            Self::Default => f.write_str("default"),
        }
    }
}

/// All inputs; loading never reads the process environment or discovers a worktree.
#[derive(Debug, Clone)]
pub struct LoadOptions {
    /// CLI whose files, keys and environment prefix are accepted.
    pub cli: CliName,
    /// Home containing the global configuration folder.
    pub home: PathBuf,
    /// Git worktree root; absent means skip local layers entirely.
    pub worktree: Option<PathBuf>,
    /// Default supplied by the caller via `sc_agent_types::default_log_root`
    /// (also re-exported as `sc_agent_log::default_log_root`).
    pub default_log_root: PathBuf,
    /// Dotted key paths to flag values.
    pub flags: BTreeMap<String, Value>,
    /// Optional flag spelling, including `--`, per dotted path; absent paths
    /// display as `--` plus the path with `_` replaced by `-`.
    pub flag_names: BTreeMap<String, String>,
    /// The only environment values the loader can read.
    pub environment: Vec<(OsString, OsString)>,
}
/// A configuration layer file, whether present or missing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LayerFile {
    /// Its full path.
    pub path: PathBuf,
    /// Whether it exists.
    pub exists: bool,
}
/// Validated configuration together with its provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    /// Resolved values.
    pub config: AgentConfig,
    /// Dotted setting path to winning layer.
    pub sources: BTreeMap<String, Source>,
    /// Files in lookup order (local pair, then global pair).
    pub files: Vec<LayerFile>,
}
/// A JSON-ready doctor table row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorRow {
    /// Dotted setting path.
    pub setting: String,
    /// Human-readable resolved value.
    pub value: String,
    /// Flag, variable, file path, or `default`.
    pub source: String,
}

fn invalid(path: &str, message: impl Into<String>) -> AgentConfigError {
    AgentConfigError::Invalid {
        path: path.into(),
        message: message.into(),
    }
}
fn at(source: &Source, cause: AgentConfigError) -> AgentConfigError {
    AgentConfigError::Layer {
        source: source.to_string(),
        cause: Box::new(cause),
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EnvValue {
    String,
    Json,
}

#[derive(Clone, Copy)]
struct Setting {
    path: &'static str,
    shared: bool,
    env: EnvValue,
}

// The loader's schema for accepted paths and environment value types; doctor
// rows walk its top-level roots. `*` matches one dynamic model or runner name.
const SETTINGS: &[Setting] = &[
    Setting {
        path: "project",
        shared: true,
        env: EnvValue::String,
    },
    Setting {
        path: "log_root",
        shared: true,
        env: EnvValue::String,
    },
    Setting {
        path: "workers",
        shared: false,
        env: EnvValue::Json,
    },
    Setting {
        path: "max_sends",
        shared: false,
        env: EnvValue::Json,
    },
    Setting {
        path: "models",
        shared: false,
        env: EnvValue::Json,
    },
    Setting {
        path: "models.*",
        shared: false,
        env: EnvValue::Json,
    },
    Setting {
        path: "models.*.line",
        shared: false,
        env: EnvValue::String,
    },
    Setting {
        path: "models.*.released",
        shared: false,
        env: EnvValue::String,
    },
    Setting {
        path: "local_runners",
        shared: false,
        env: EnvValue::Json,
    },
    Setting {
        path: "local_runners.*",
        shared: false,
        env: EnvValue::Json,
    },
    Setting {
        path: "local_runners.*.kind",
        shared: false,
        env: EnvValue::String,
    },
    Setting {
        path: "local_runners.*.host",
        shared: false,
        env: EnvValue::String,
    },
    Setting {
        path: "local_runners.*.port",
        shared: false,
        env: EnvValue::Json,
    },
    Setting {
        path: "local_runners.*.model",
        shared: false,
        env: EnvValue::String,
    },
    Setting {
        path: "local_runners.*.key_env",
        shared: false,
        env: EnvValue::String,
    },
    Setting {
        path: "local_runners.*.request_timeout_secs",
        shared: false,
        env: EnvValue::Json,
    },
    Setting {
        path: "local_runners.*.load_timeout_secs",
        shared: false,
        env: EnvValue::Json,
    },
];

fn path_matches(pattern: &str, path: &[&str], allow_prefix: bool) -> bool {
    let parts: Vec<_> = pattern.split('.').collect();
    (allow_prefix && path.len() <= parts.len() || path.len() == parts.len())
        && path
            .iter()
            .zip(parts)
            .all(|(actual, expected)| expected == "*" || actual == &expected)
}

fn check_path(path: &[&str], cli: CliName, shared: bool) -> Result<(), AgentConfigError> {
    if SETTINGS.iter().any(|setting| {
        let scope_ok = if shared {
            setting.shared
        } else {
            cli == CliName::Decidr || setting.shared
        };
        scope_ok && path_matches(setting.path, path, true)
    }) {
        Ok(())
    } else {
        Err(invalid(
            path.last().copied().unwrap_or("$"),
            "setting is not accepted in this scope",
        ))
    }
}

fn check_object(
    value: &Value,
    path: &mut Vec<String>,
    cli: CliName,
    shared: bool,
) -> Result<(), AgentConfigError> {
    let object = value.as_object().ok_or_else(|| {
        invalid(
            path.last().map_or("$", String::as_str),
            "configuration must be a JSON object",
        )
    })?;
    for (key, child) in object {
        path.push(key.clone());
        let parts: Vec<_> = path.iter().map(String::as_str).collect();
        check_path(&parts, cli, shared)?;
        if child.is_object() {
            check_object(child, path, cli, shared)?;
        }
        path.pop();
    }
    Ok(())
}

fn check_keys(value: &Value, cli: CliName, shared: bool) -> Result<(), AgentConfigError> {
    check_object(value, &mut Vec::new(), cli, shared)
}

fn env_value_kind(path: &str) -> EnvValue {
    let parts: Vec<_> = path.split('.').collect();
    SETTINGS
        .iter()
        .find(|setting| path_matches(setting.path, &parts, false))
        .map_or(EnvValue::Json, |setting| setting.env)
}
fn file_value(path: &Path) -> Result<Option<Value>, AgentConfigError> {
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(cause) => {
            return Err(AgentConfigError::Read {
                file: path.into(),
                cause,
            });
        }
    };
    if !metadata.is_file() {
        return Err(AgentConfigError::NotAFile { file: path.into() });
    }
    if metadata.len() > MAX_CONFIG_FILE_BYTES {
        return Err(AgentConfigError::FileTooLarge {
            file: path.into(),
            bytes: metadata.len(),
        });
    }
    let file = std::fs::File::open(path).map_err(|cause| AgentConfigError::Read {
        file: path.into(),
        cause,
    })?;
    let mut bytes = Vec::new();
    file.take(MAX_CONFIG_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|cause| AgentConfigError::Read {
            file: path.into(),
            cause,
        })?;
    if bytes.len() as u64 > MAX_CONFIG_FILE_BYTES {
        return Err(AgentConfigError::FileTooLarge {
            file: path.into(),
            bytes: bytes.len() as u64,
        });
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|cause| AgentConfigError::Json {
            file: path.into(),
            cause,
        })
}
fn put(root: &mut Value, parts: &[&str], value: Value) -> Result<(), AgentConfigError> {
    let Some((key, rest)) = parts.split_first() else {
        return Err(invalid("$", "empty key path"));
    };
    if key.is_empty() {
        return Err(invalid("$", "empty key in setting path"));
    }
    let object = root
        .as_object_mut()
        .ok_or_else(|| invalid(key, "nested setting conflicts with a scalar"))?;
    if rest.is_empty() {
        object.insert((*key).into(), value);
        return Ok(());
    }
    put(
        object
            .entry((*key).to_owned())
            .or_insert_with(|| Value::Object(Map::new())),
        rest,
        value,
    )
}
fn merge(
    into: &mut Value,
    value: &Value,
    path: &str,
    source: &Source,
    sources: &mut BTreeMap<String, Source>,
) {
    if let (Some(dst), Some(src)) = (into.as_object_mut(), value.as_object()) {
        if src.is_empty() && dst.is_empty() && !path.is_empty() {
            sources.insert(path.into(), source.clone());
        }
        for (key, val) in src {
            let next = if path.is_empty() {
                key.clone()
            } else {
                format!("{path}.{key}")
            };
            merge(
                dst.entry(key.clone()).or_insert(Value::Null),
                val,
                &next,
                source,
                sources,
            );
        }
    } else {
        *into = value.clone();
        let prefix = format!("{path}.");
        sources.retain(|key, _| key != path && !key.starts_with(&prefix));
        if let Some(obj) = value.as_object() {
            for (key, val) in obj {
                let next = format!("{path}.{key}");
                let mut ignored = Value::Null;
                merge(&mut ignored, val, &next, source, sources);
            }
            if obj.is_empty() {
                sources.insert(path.into(), source.clone());
            }
        } else {
            sources.insert(path.into(), source.clone());
        }
    }
}
fn decode(mut value: Value, cli: CliName) -> Result<AgentConfig, AgentConfigError> {
    if value.get("log_root").is_some_and(|v| !v.is_string()) {
        return Err(invalid("log_root", "log_root must be a path string"));
    }
    let project = value.as_object_mut().and_then(|obj| obj.remove("project"));
    let project = match project {
        None => None,
        Some(Value::String(s)) if sc_agent_types::is_valid_slug(&s) => Some(s),
        Some(_) => {
            return Err(invalid(
                "project",
                format!(
                    "project/phase must be a path-safe lowercase slug using a-z, 0-9, '.', '_' or '-', at most {} bytes, not '.', '..', a trailing dot or a Windows device name",
                    sc_agent_types::MAX_SEGMENT_BYTES
                ),
            ));
        }
    };
    let mut config: AgentConfig =
        serde_path_to_error::deserialize(value).map_err(crate::config::invalid)?;
    crate::config::check_runners(&config)?;
    config.project = project;
    config.cli = cli;
    Ok(config)
}

// The provenance of an error path: its own source or that of its nearest recorded parent.
fn source_for<'a>(
    error: &AgentConfigError,
    sources: &'a BTreeMap<String, Source>,
    fallback: &'a Source,
) -> &'a Source {
    let (AgentConfigError::Invalid { path, .. }
    | AgentConfigError::RunnerName { path, .. }
    | AgentConfigError::KeyOnPlainHttp { path, .. }) = error
    else {
        return fallback;
    };
    sources
        .iter()
        .filter(|(key, _)| {
            path.strip_prefix(key.as_str())
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
        })
        .max_by_key(|(key, _)| key.len())
        .map_or(fallback, |(_, source)| source)
}

/// Loads the seven layers, validating every supplied input even if a higher layer wins.
///
/// # Errors
/// Returns typed file, JSON, scope or bounds errors, preserving the originating
/// file/variable/flag and the underlying validation error. No file is written.
pub fn load_layered(options: &LoadOptions) -> Result<Loaded, AgentConfigError> {
    let global = sc_agent_types::default_log_root(&options.home);
    let mut paths = Vec::new();
    if let Some(worktree) = &options.worktree {
        let local = worktree.join(".sc").join("sc-agent");
        paths.extend([local.join(options.cli.file_name()), local.join(SHARED_FILE)]);
    }
    paths.extend([
        global.join(options.cli.file_name()),
        global.join(SHARED_FILE),
    ]);
    let mut files = Vec::new();
    let mut layers = Vec::new();
    for path in &paths {
        let source = Source::File(path.clone());
        let input = file_value(path)?;
        files.push(LayerFile {
            path: path.clone(),
            exists: input.is_some(),
        });
        if let Some(value) = input {
            check_keys(
                &value,
                options.cli,
                path.file_name().is_some_and(|s| s == SHARED_FILE),
            )
            .map_err(|e| at(&source, e))?;
            layers.push((value, source));
        }
    }
    layers.reverse();
    for (name, raw) in &options.environment {
        let Some(name) = name.to_str() else {
            continue;
        };
        let key = if name == PROJECT_ENV {
            "project".to_owned()
        } else if let Some(suffix) = name.strip_prefix(options.cli.env_prefix()) {
            if suffix == "PROJECT" {
                continue;
            }
            suffix.to_ascii_lowercase().replace("__", ".")
        } else {
            continue;
        };
        let source = Source::Env(name.into());
        let text = raw
            .to_str()
            .ok_or_else(|| at(&source, invalid(&key, "value is not UTF-8")))?;
        let value = match env_value_kind(&key) {
            EnvValue::String => Value::String(text.into()),
            EnvValue::Json => {
                serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.into()))
            }
        };
        let mut object = Value::Object(Map::new());
        put(&mut object, &key.split('.').collect::<Vec<_>>(), value).map_err(|e| at(&source, e))?;
        check_keys(&object, options.cli, false).map_err(|e| at(&source, e))?;
        layers.push((object, source));
    }
    for (key, value) in &options.flags {
        let source = Source::Flag(
            options
                .flag_names
                .get(key)
                .cloned()
                .unwrap_or_else(|| format!("--{}", key.replace('_', "-"))),
        );
        let mut object = Value::Object(Map::new());
        put(
            &mut object,
            &key.split('.').collect::<Vec<_>>(),
            value.clone(),
        )
        .map_err(|e| at(&source, e))?;
        check_keys(&object, options.cli, false).map_err(|e| at(&source, e))?;
        layers.push((object, source));
    }

    let mut value = Value::Object(Map::new());
    let mut sources = BTreeMap::new();
    sources.insert("log_root".into(), Source::Default);
    for (layer, source) in &layers {
        merge(&mut value, layer, "", source, &mut sources);
    }
    // Decode the winning object, then each layer re-applied over it, so a masked
    // invalid value still fails under its own source.
    let mut config = decode(value.clone(), options.cli).map_err(|error| {
        let top = layers.last().map_or(&Source::Default, |(_, source)| source);
        at(source_for(&error, &sources, top), error)
    })?;
    for (layer, source) in layers.iter().rev().skip(1) {
        let mut checked = value.clone();
        let mut checked_sources = sources.clone();
        merge(&mut checked, layer, "", source, &mut checked_sources);
        decode(checked, options.cli)
            .map_err(|error| at(source_for(&error, &checked_sources, source), error))?;
    }
    if config.log_root.is_none() {
        config.log_root = Some(options.default_log_root.clone());
    }
    let mut loaded = Loaded {
        config,
        sources,
        files,
    };
    for row in doctor_rows(&loaded) {
        loaded.sources.entry(row.setting).or_insert(Source::Default);
    }
    Ok(loaded)
}

/// One row per resolved leaf of the serialized config this CLI accepts, shared
/// settings first, each with its recorded source. This performs no I/O.
#[must_use]
pub fn doctor_rows(loaded: &Loaded) -> Vec<DoctorRow> {
    let config = serde_json::to_value(&loaded.config).unwrap_or_default();
    let mut rows = Vec::new();
    for root in SETTINGS.iter().map(|s| s.path).filter(|p| !p.contains('.')) {
        if let Some(value) = config.get(root)
            && check_path(&[root], loaded.config.cli, false).is_ok()
        {
            walk(value, root.to_owned(), &loaded.sources, &mut rows);
        }
    }
    rows
}

fn walk(
    value: &Value,
    path: String,
    sources: &BTreeMap<String, Source>,
    rows: &mut Vec<DoctorRow>,
) {
    if let Some(fields) = value.as_object().filter(|fields| !fields.is_empty()) {
        for (key, child) in fields {
            walk(child, format!("{path}.{key}"), sources, rows);
        }
        return;
    }
    let value = match value {
        Value::String(text) => text.clone(),
        Value::Null => "unset".into(),
        other => other.to_string(),
    };
    let source = sources.get(&path).unwrap_or(&Source::Default).to_string();
    rows.push(DoctorRow {
        setting: path,
        value,
        source,
    });
}
