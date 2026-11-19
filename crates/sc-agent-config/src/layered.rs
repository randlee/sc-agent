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
// A validation pass may fail on a different layer's winning value. Attribute it
// using the checked object's provenance, rather than the layer currently overlaid.
fn decode_source<'a>(
    error: &AgentConfigError,
    sources: &'a BTreeMap<String, Source>,
    fallback: &'a Source,
    precedence: &[&Source],
) -> &'a Source {
    let path = match error {
        AgentConfigError::Invalid { path, .. }
        | AgentConfigError::RunnerName { path, .. }
        | AgentConfigError::KeyOnPlainHttp { path, .. } => path.as_str(),
        _ => return fallback,
    };
    // Missing fields and container limits are reported at the parent path. That
    // parent may have no source entry because the merger records its leaf values.
    if let Some((_, source)) = sources
        .iter()
        .filter(|(key, _)| {
            key.strip_prefix(path)
                .is_some_and(|rest| rest.starts_with('.') || rest.starts_with('['))
        })
        .max_by_key(|(_, source)| {
            precedence
                .iter()
                .rposition(|candidate| *candidate == *source)
        })
    {
        return source;
    }
    sources
        .iter()
        .filter(|(key, _)| {
            path == key.as_str()
                || path
                    .strip_prefix(key.as_str())
                    .is_some_and(|rest| rest.starts_with('.') || rest.starts_with('['))
        })
        .max_by_key(|(key, _)| key.len())
        .map_or(fallback, |(_, source)| source)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EnvValue {
    String,
    Json,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DoctorValue {
    None,
    Project,
    LogRoot,
    Workers,
    MaxSends,
    EmptyModels,
    ModelLine,
    ModelReleased,
    EmptyRunners,
    RunnerKind,
    RunnerHost,
    RunnerPort,
    RunnerModel,
    RunnerKeyEnv,
    RunnerRequestTimeout,
    RunnerLoadTimeout,
}

#[derive(Clone, Copy)]
struct Setting {
    path: &'static str,
    shared: bool,
    env: EnvValue,
    doctor: DoctorValue,
}

// This is the loader's schema for accepted paths, environment value types and
// doctor rows. `*` matches one dynamic model or runner name.
const SETTINGS: &[Setting] = &[
    Setting {
        path: "project",
        shared: true,
        env: EnvValue::String,
        doctor: DoctorValue::Project,
    },
    Setting {
        path: "log_root",
        shared: true,
        env: EnvValue::String,
        doctor: DoctorValue::LogRoot,
    },
    Setting {
        path: "workers",
        shared: false,
        env: EnvValue::Json,
        doctor: DoctorValue::Workers,
    },
    Setting {
        path: "max_sends",
        shared: false,
        env: EnvValue::Json,
        doctor: DoctorValue::MaxSends,
    },
    Setting {
        path: "models",
        shared: false,
        env: EnvValue::Json,
        doctor: DoctorValue::EmptyModels,
    },
    Setting {
        path: "models.*",
        shared: false,
        env: EnvValue::Json,
        doctor: DoctorValue::None,
    },
    Setting {
        path: "models.*.line",
        shared: false,
        env: EnvValue::String,
        doctor: DoctorValue::ModelLine,
    },
    Setting {
        path: "models.*.released",
        shared: false,
        env: EnvValue::String,
        doctor: DoctorValue::ModelReleased,
    },
    Setting {
        path: "local_runners",
        shared: false,
        env: EnvValue::Json,
        doctor: DoctorValue::EmptyRunners,
    },
    Setting {
        path: "local_runners.*",
        shared: false,
        env: EnvValue::Json,
        doctor: DoctorValue::None,
    },
    Setting {
        path: "local_runners.*.kind",
        shared: false,
        env: EnvValue::String,
        doctor: DoctorValue::RunnerKind,
    },
    Setting {
        path: "local_runners.*.host",
        shared: false,
        env: EnvValue::String,
        doctor: DoctorValue::RunnerHost,
    },
    Setting {
        path: "local_runners.*.port",
        shared: false,
        env: EnvValue::Json,
        doctor: DoctorValue::RunnerPort,
    },
    Setting {
        path: "local_runners.*.model",
        shared: false,
        env: EnvValue::String,
        doctor: DoctorValue::RunnerModel,
    },
    Setting {
        path: "local_runners.*.key_env",
        shared: false,
        env: EnvValue::String,
        doctor: DoctorValue::RunnerKeyEnv,
    },
    Setting {
        path: "local_runners.*.request_timeout_secs",
        shared: false,
        env: EnvValue::Json,
        doctor: DoctorValue::RunnerRequestTimeout,
    },
    Setting {
        path: "local_runners.*.load_timeout_secs",
        shared: false,
        env: EnvValue::Json,
        doctor: DoctorValue::RunnerLoadTimeout,
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

/// Loads the seven layers, validating every supplied input even if a higher layer wins.
///
/// # Errors
/// Returns typed file, JSON, scope or bounds errors, preserving the originating
/// file/variable/flag and the underlying validation error. No file is written.
pub fn load_layered(options: &LoadOptions) -> Result<Loaded, AgentConfigError> {
    let global = options.home.join(".sc-agent");
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
    let precedence: Vec<&Source> = layers.iter().map(|(_, source)| source).collect();
    let default_source = Source::Default;
    let fallback = layers.last().map_or(&default_source, |(_, source)| source);
    // Diagnose the winning object first. Replacing a winning leaf while checking
    // a lower layer must not hide that leaf's source on a parent-path error.
    let mut config = decode(value.clone(), options.cli).map_err(|error| {
        at(
            decode_source(&error, &sources, fallback, &precedence),
            error,
        )
    })?;
    // Validate lower values against the full object, so partial nested overrides
    // keep their required siblings while masked invalid values still fail.
    for (layer, source) in &layers {
        let mut checked = value.clone();
        let mut checked_sources = sources.clone();
        merge(&mut checked, layer, "", source, &mut checked_sources);
        decode(checked, options.cli).map_err(|error| {
            at(
                decode_source(&error, &checked_sources, source, &precedence),
                error,
            )
        })?;
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

/// Shared settings first, then deterministic CLI-specific rows. This performs no I/O.
#[must_use]
pub fn doctor_rows(loaded: &Loaded) -> Vec<DoctorRow> {
    let c = &loaded.config;
    let mut rows = Vec::new();
    let mut push = |setting: String, value: String| {
        let source = loaded
            .sources
            .get(&setting)
            .unwrap_or(&Source::Default)
            .to_string();
        rows.push(DoctorRow {
            setting,
            value,
            source,
        });
    };
    for spec in SETTINGS {
        match spec.doctor {
            DoctorValue::Project => push("project".into(), c.project().unwrap_or("unset").into()),
            DoctorValue::LogRoot => push(
                "log_root".into(),
                c.log_root
                    .as_ref()
                    .map_or_else(|| "unset".into(), |p| p.display().to_string()),
            ),
            DoctorValue::Workers if c.cli == CliName::Decidr => {
                push("workers".into(), c.workers.to_string())
            }
            DoctorValue::MaxSends if c.cli == CliName::Decidr => {
                push("max_sends".into(), c.max_sends.to_string())
            }
            DoctorValue::EmptyModels if c.cli == CliName::Decidr && c.models.is_empty() => {
                push("models".into(), "{}".into())
            }
            DoctorValue::EmptyRunners if c.cli == CliName::Decidr && c.local_runners.is_empty() => {
                push("local_runners".into(), "{}".into())
            }
            _ => {}
        }
    }
    if c.cli == CliName::Decidr {
        for (id, model) in &c.models {
            for spec in SETTINGS.iter().filter(|s| s.path.starts_with("models.*.")) {
                match spec.doctor {
                    DoctorValue::ModelLine => {
                        push(format!("models.{id}.line"), model.line.as_str().into())
                    }
                    DoctorValue::ModelReleased => {
                        push(format!("models.{id}.released"), model.released.to_string())
                    }
                    _ => {}
                }
            }
        }
        for (name, runner) in &c.local_runners {
            for spec in SETTINGS
                .iter()
                .filter(|s| s.path.starts_with("local_runners.*."))
            {
                let prefix = format!("local_runners.{name}");
                match spec.doctor {
                    DoctorValue::RunnerKind => {
                        let kind = match runner.kind {
                            RunnerKind::Laya => "laya",
                            RunnerKind::Strands => "strands",
                            RunnerKind::Mapika => "mapika",
                            RunnerKind::Generic => "generic",
                        };
                        push(format!("{prefix}.kind"), kind.into());
                    }
                    DoctorValue::RunnerHost => {
                        push(format!("{prefix}.host"), runner.host.as_str().into())
                    }
                    DoctorValue::RunnerPort => {
                        push(format!("{prefix}.port"), runner.port.to_string())
                    }
                    DoctorValue::RunnerModel => {
                        push(format!("{prefix}.model"), runner.model.to_string())
                    }
                    DoctorValue::RunnerKeyEnv => push(
                        format!("{prefix}.key_env"),
                        runner
                            .key_env
                            .as_ref()
                            .map_or_else(|| "unset".into(), ToString::to_string),
                    ),
                    DoctorValue::RunnerRequestTimeout => push(
                        format!("{prefix}.request_timeout_secs"),
                        runner.request_timeout().as_secs().to_string(),
                    ),
                    DoctorValue::RunnerLoadTimeout => push(
                        format!("{prefix}.load_timeout_secs"),
                        runner.load_timeout().as_secs().to_string(),
                    ),
                    _ => {}
                }
            }
        }
    }
    rows
}

#[cfg(test)]
mod schema_tests {
    use super::*;

    fn collect_leaf_paths(value: &Value, prefix: &mut Vec<String>, out: &mut Vec<String>) {
        if let Some(object) = value.as_object() {
            for (key, child) in object {
                prefix.push(key.clone());
                collect_leaf_paths(child, prefix, out);
                prefix.pop();
            }
        } else {
            out.push(prefix.join("."));
        }
    }

    fn assert_string_leaf_kinds(value: &Value, path: &mut Vec<String>) {
        if let Some(object) = value.as_object() {
            for (key, child) in object {
                path.push(key.clone());
                assert_string_leaf_kinds(child, path);
                path.pop();
            }
        } else if value.is_string() {
            let path = path.join(".");
            assert_eq!(
                env_value_kind(&path),
                EnvValue::String,
                "string leaf {path}"
            );
        }
    }

    #[test]
    fn populated_config_emits_each_declared_doctor_row_and_string_kind() {
        let config = decode(
            serde_json::json!({
                "project": "phase-a", "log_root": "/controlled",
                "models": {"123": {"line": "haiku", "released": "2026-01-01"}},
                "local_runners": {"fixture": {
                    "kind": "generic", "host": "localhost", "port": 8123,
                    "model": "123", "key_env": "TOKEN", "request_timeout_secs": 30,
                    "load_timeout_secs": 60
                }}
            }),
            CliName::Decidr,
        )
        .unwrap();
        let serialized = serde_json::to_value(&config).unwrap();
        assert_string_leaf_kinds(&serialized, &mut Vec::new());
        assert_eq!(env_value_kind("project"), EnvValue::String);
        let loaded = Loaded {
            config,
            sources: BTreeMap::new(),
            files: Vec::new(),
        };
        let empty = Loaded {
            config: AgentConfig::default(),
            sources: BTreeMap::new(),
            files: Vec::new(),
        };
        let populated_rows = doctor_rows(&loaded);
        let empty_rows = doctor_rows(&empty);
        for setting in SETTINGS
            .iter()
            .filter(|setting| setting.doctor != DoctorValue::None)
        {
            let path = setting
                .path
                .replace("models.*.", "models.123.")
                .replace("local_runners.*.", "local_runners.fixture.");
            let rows = if matches!(
                setting.doctor,
                DoctorValue::EmptyModels | DoctorValue::EmptyRunners
            ) {
                &empty_rows
            } else {
                &populated_rows
            };
            assert!(
                rows.iter().any(|row| row.setting == path),
                "missing doctor row for {}",
                setting.path
            );
        }
    }

    #[test]
    fn schema_covers_every_serialized_config_field_and_doctor_row() {
        let config = decode(
            serde_json::json!({
                "models": {"123": {"line": "haiku", "released": "2026-01-01"}},
                "local_runners": {"fixture": {
                    "kind": "generic", "host": "localhost", "port": 8123,
                    "model": "123", "key_env": "TOKEN", "request_timeout_secs": 30,
                    "load_timeout_secs": 60
                }}
            }),
            CliName::Decidr,
        )
        .unwrap();
        let serialized = serde_json::to_value(config).unwrap();
        let actual_roots: std::collections::BTreeSet<_> = serialized
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let schema_roots: std::collections::BTreeSet<_> = SETTINGS
            .iter()
            .filter_map(|setting| setting.path.split('.').next())
            .filter(|root| *root != "project")
            .collect();
        assert_eq!(actual_roots, schema_roots);
        assert!(SETTINGS.iter().any(|setting| setting.path == "project"));
        for root in &actual_roots {
            assert!(
                SETTINGS.iter().any(|setting| {
                    setting.path.split('.').next() == Some(root)
                        && setting.doctor != DoctorValue::None
                }),
                "missing doctor row definition for AgentConfig field {root}"
            );
        }

        let mut actual_paths = Vec::new();
        collect_leaf_paths(&serialized, &mut Vec::new(), &mut actual_paths);
        for path in actual_paths {
            let parts: Vec<_> = path.split('.').collect();
            assert!(
                SETTINGS
                    .iter()
                    .any(|setting| path_matches(setting.path, &parts, false)),
                "missing schema path {path}"
            );
        }
        for setting in SETTINGS
            .iter()
            .filter(|setting| setting.path.contains("*."))
        {
            if setting.path.contains(".*.") {
                assert_ne!(
                    setting.doctor,
                    DoctorValue::None,
                    "missing doctor row for {}",
                    setting.path
                );
            }
        }
    }
}
