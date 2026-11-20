//! Explicit-input configuration layers shared by the CLIs.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::{AgentConfig, AgentConfigError, MAX_CONFIG_FILE_BYTES, RunnerKind};

/// Shared-scope file in each configuration folder.
pub const SHARED_FILE: &str = "sc-agent.json";
/// The project/phase variable shared by every CLI.
pub const PROJECT_ENV: &str = "SC_PROJECT";

/// The closed CLI scope table of ADR-SAG-0020.
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
    /// Default supplied by the caller's shared default-log-root function.
    pub default_log_root: PathBuf,
    /// Dotted key paths to flag values.
    pub flags: BTreeMap<String, Value>,
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
fn check_keys(value: &Value, cli: CliName, shared: bool) -> Result<(), AgentConfigError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("$", "configuration must be a JSON object"))?;
    for key in object.keys() {
        if !matches!(key.as_str(), "project" | "log_root")
            && (shared
                || cli == CliName::Sitr
                || !matches!(
                    key.as_str(),
                    "models" | "workers" | "max_sends" | "local_runners"
                ))
        {
            return Err(invalid(key, "setting is not accepted in this scope"));
        }
    }
    Ok(())
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

/// Loads the seven layers, validating every supplied input even if a higher layer wins.
///
/// # Errors
/// Returns typed file, JSON, scope or bounds errors, preserving the originating
/// file/variable/flag and the underlying validation error. No file is written.
pub fn load_layered(options: &LoadOptions) -> Result<Loaded, AgentConfigError> {
    let global = options.home.join(".sc-agent");
    let mut paths = Vec::new();
    if let Some(worktree) = &options.worktree {
        let local = worktree.join(".sc/sc-agent");
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
        let leaf = key.rsplit('.').next().unwrap_or(&key);
        let value = if matches!(
            leaf,
            "project" | "log_root" | "kind" | "host" | "model" | "key_env" | "line" | "released"
        ) {
            Value::String(text.into())
        } else {
            serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.into()))
        };
        let mut object = Value::Object(Map::new());
        put(&mut object, &key.split('.').collect::<Vec<_>>(), value).map_err(|e| at(&source, e))?;
        check_keys(&object, options.cli, false).map_err(|e| at(&source, e))?;
        layers.push((object, source));
    }
    for (key, value) in &options.flags {
        let source = Source::Flag(format!("--{}", key.replace('_', "-")));
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
    // Validate lower values against the full object, so partial nested overrides
    // keep their required siblings while masked invalid values still fail.
    for (layer, source) in &layers {
        let mut checked = value.clone();
        merge(&mut checked, layer, "", source, &mut BTreeMap::new());
        decode(checked, options.cli).map_err(|e| at(source, e))?;
    }
    let mut config = decode(value, options.cli)?;
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

/// Shared settings first, then deterministic CLI-specific rows. This performs no I/O.
#[must_use]
pub fn doctor_rows(loaded: &Loaded) -> Vec<DoctorRow> {
    let c = &loaded.config;
    let mut values = vec![
        ("project".into(), c.project().unwrap_or("unset").to_owned()),
        (
            "log_root".into(),
            c.log_root
                .as_ref()
                .map_or_else(|| "unset".into(), |p| p.display().to_string()),
        ),
    ];
    if c.cli == CliName::Decidr {
        values.extend([
            ("workers".into(), c.workers.to_string()),
            ("max_sends".into(), c.max_sends.to_string()),
        ]);
        if c.models.is_empty() {
            values.push(("models".into(), "{}".into()));
        }
        for (id, model) in &c.models {
            values.extend([
                (format!("models.{id}.line"), model.line.as_str().into()),
                (format!("models.{id}.released"), model.released.to_string()),
            ]);
        }
        if c.local_runners.is_empty() {
            values.push(("local_runners".into(), "{}".into()));
        }
        for (name, r) in &c.local_runners {
            let prefix = format!("local_runners.{name}");
            let kind = match r.kind {
                RunnerKind::Laya => "laya",
                RunnerKind::Strands => "strands",
                RunnerKind::Mapika => "mapika",
                RunnerKind::Generic => "generic",
            };
            for (key, value) in [
                ("kind", kind.into()),
                ("host", r.host.as_str().into()),
                ("port", r.port.to_string()),
                ("model", r.model.to_string()),
                (
                    "key_env",
                    r.key_env
                        .as_ref()
                        .map_or_else(|| "unset".into(), ToString::to_string),
                ),
                (
                    "request_timeout_secs",
                    r.request_timeout().as_secs().to_string(),
                ),
                ("load_timeout_secs", r.load_timeout().as_secs().to_string()),
            ] {
                values.push((format!("{prefix}.{key}"), value));
            }
        }
    }
    values
        .into_iter()
        .map(|(setting, value)| {
            let source = loaded
                .sources
                .get(&setting)
                .unwrap_or(&Source::Default)
                .to_string();
            DoctorRow {
                setting,
                value,
                source,
            }
        })
        .collect()
}
