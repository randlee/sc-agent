//! Explicit-input configuration layers shared by the CLIs.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;

use serde::Serialize;
use serde_json::Value;

use crate::layers::{self, EnvValue, FileLayer, Inputs, Key, Layered, LayersError};
use crate::{AgentConfig, AgentConfigError};

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

/// All inputs; loading never reads the process environment or discovers a worktree.
#[derive(Debug, Clone)]
pub struct LoadOptions {
    /// CLI whose files, keys and environment prefix are accepted.
    pub cli: CliName,
    /// Home containing the global configuration folder.
    pub home: PathBuf,
    /// Git worktree root; absent means skip local layers entirely.
    pub worktree: Option<PathBuf>,
    /// Dotted key paths to flag values.
    pub flags: BTreeMap<String, Value>,
    /// Optional flag spelling, including `--`, per dotted path; absent paths
    /// display as `--` plus the path with `_` replaced by `-`.
    pub flag_names: BTreeMap<String, String>,
    /// The only environment values the loader can read.
    pub environment: Vec<(OsString, OsString)>,
}
/// Validated configuration together with its provenance.
pub type Loaded = Layered<AgentConfig>;
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

impl From<LayersError<AgentConfigError>> for AgentConfigError {
    fn from(error: LayersError<AgentConfigError>) -> Self {
        match error {
            LayersError::Read { file, cause } => Self::Read { file, cause },
            LayersError::Json { file, cause } => Self::Json { file, cause },
            LayersError::TooLarge { file, bytes } => Self::FileTooLarge { file, bytes },
            LayersError::NotAFile { file } => Self::NotAFile { file },
            LayersError::InvalidKey { path, message } => Self::Invalid { path, message },
            LayersError::At { source, cause } => Self::Layer {
                source: source.to_string(),
                cause: Box::new((*cause).into()),
            },
            LayersError::Decode(error) => error,
        }
    }
}

const fn key(path: &'static str, env: EnvValue) -> Key {
    Key { path, env }
}

// Decidr's accepted keys, shared first; sitr accepts only the shared ones.
// `*` matches one dynamic model or runner name.
const DECIDR_KEYS: &[Key] = &[
    key("project", EnvValue::String),
    key("workers", EnvValue::Json),
    key("max_sends", EnvValue::Json),
    key("models", EnvValue::Json),
    key("models.*", EnvValue::Json),
    key("models.*.line", EnvValue::String),
    key("models.*.released", EnvValue::String),
    key("local_runners", EnvValue::Json),
    key("local_runners.*", EnvValue::Json),
    key("local_runners.*.kind", EnvValue::String),
    key("local_runners.*.host", EnvValue::String),
    key("local_runners.*.port", EnvValue::Json),
    key("local_runners.*.model", EnvValue::String),
    key("local_runners.*.key_env", EnvValue::String),
    key("local_runners.*.request_timeout_secs", EnvValue::Json),
    key("local_runners.*.load_timeout_secs", EnvValue::Json),
];
const SHARED_KEYS: &[Key] = DECIDR_KEYS.split_at(1).0;

/// The keys `cli` accepts, and the keys its prefixed variables may name.
fn keys(cli: CliName) -> (&'static [Key], &'static [Key]) {
    match cli {
        CliName::Decidr => (DECIDR_KEYS, DECIDR_KEYS.split_at(1).1),
        CliName::Sitr => (SHARED_KEYS, &[]),
    }
}

fn decode(mut value: Value, cli: CliName) -> Result<AgentConfig, AgentConfigError> {
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

/// The key path of a decode error, which names the layer that supplied it.
fn decode_path(error: &AgentConfigError) -> Option<&str> {
    match error {
        AgentConfigError::Invalid { path, .. }
        | AgentConfigError::RunnerName { path, .. }
        | AgentConfigError::KeyOnPlainHttp { path, .. } => Some(path),
        _ => None,
    }
}

/// Loads the seven layers, validating every supplied input even if a higher layer wins.
///
/// # Errors
/// Returns typed file, JSON, scope or bounds errors, preserving the originating
/// file/variable/flag and the underlying validation error. No file is written.
pub fn load_layered(options: &LoadOptions) -> Result<Loaded, AgentConfigError> {
    let (accepts, env_accepts) = keys(options.cli);
    let global = sc_agent_types::default_log_root(&options.home);
    let mut folders = Vec::new();
    if let Some(worktree) = &options.worktree {
        folders.push(worktree.join(".sc").join("sc-agent"));
    }
    folders.push(global);
    let files = folders
        .iter()
        .flat_map(|folder| {
            [
                FileLayer {
                    path: folder.join(options.cli.file_name()),
                    accepts,
                },
                FileLayer {
                    path: folder.join(SHARED_FILE),
                    accepts: SHARED_KEYS,
                },
            ]
        })
        .collect();
    let inputs = Inputs {
        files,
        env_prefix: options.cli.env_prefix(),
        env_aliases: &[(PROJECT_ENV, "project")],
        env_accepts,
        flags: options.flags.clone(),
        flag_names: options.flag_names.clone(),
        keys: accepts,
        environment: &options.environment,
    };
    Ok(layers::load(
        &inputs,
        |value| decode(value, options.cli),
        decode_path,
    )?)
}

/// One row per resolved leaf of the serialized config this CLI accepts, shared
/// settings first, each with its recorded source. This performs no I/O.
#[must_use]
pub fn doctor_rows(loaded: &Loaded) -> Vec<DoctorRow> {
    layers::resolved(loaded, keys(loaded.config.cli).0)
        .into_iter()
        .map(|row| DoctorRow {
            setting: row.path,
            value: row.value,
            source: row.source,
        })
        .collect()
}
