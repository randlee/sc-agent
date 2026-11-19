//! The error type of sc-agent-config.

use std::fmt;
use std::path::PathBuf;

use sc_agent_types::{ErrorDetail, ErrorId, TypesError};

/// The configuration could not be loaded or used.
///
/// [`Display`](fmt::Display) writes `"<id>: <message>"`; a wrapped sc-config
/// or sc-agent-types error is in [`source`](std::error::Error::source) and
/// [`cause_detail`](ErrorId::cause_detail), never in the message.
#[derive(Debug)]
#[non_exhaustive]
pub enum AgentConfigError {
    /// No layer sets the required project/phase.
    ProjectRequired {
        /// The CLI file that can set the value.
        cli: &'static str,
    },
    /// A layered input failed validation; its original error is retained.
    Layer {
        /// The flag, environment variable or file supplying this input.
        source: String,
        /// The unchanged validation failure.
        cause: Box<AgentConfigError>,
    },
    /// Reading a layer failed.
    Read {
        /// The layer file.
        file: PathBuf,
        /// The original I/O failure.
        cause: std::io::Error,
    },
    /// A layer is not valid JSON.
    Json {
        /// The layer file.
        file: PathBuf,
        /// The original parser failure.
        cause: serde_json::Error,
    },
    /// sc-config failed: a file is unreadable or not JSON, or an override cannot be applied.
    Load(sc_config::ConfigError),
    /// A `models` entry conflicts with the catalog (a model on two lines).
    Catalog(TypesError),
    /// There is no home directory, so `~/.sc-agent` cannot be located.
    HomeUnknown,
    /// A configuration file is larger than `MAX_CONFIG_FILE_BYTES` (1 MiB).
    FileTooLarge {
        /// The file.
        file: PathBuf,
        /// Its size in bytes.
        bytes: u64,
    },
    /// A configuration path exists but is not a regular file (a FIFO, a device or a
    /// directory): it is refused before any read, which could block forever.
    NotAFile {
        /// The path.
        file: PathBuf,
    },
    /// A local runner is named like a built-in short name.
    RunnerName {
        /// The runner name.
        name: String,
        /// `local_runners.<name>`.
        path: String,
    },
    /// A value failed its type or bound (REQ-SAG-0013).
    Invalid {
        /// The key path, such as `workers` or `local_runners.laya.host`.
        path: String,
        /// The serde or sc-agent-types text.
        message: String,
    },
    /// `key_env` is set on a runner whose host is not loopback (plain http, REQ-SAG-0013).
    KeyOnPlainHttp {
        /// `local_runners.<name>.key_env`.
        path: String,
        /// The runner's host.
        host: String,
    },
}

sc_agent_types::error_catalog! { AgentConfigError {
    AgentConfigError::ProjectRequired { .. } => ("config.project_required", Some(sc_agent_types::ErrorCategory::Config)),
    AgentConfigError::Layer { .. } => ("config.layer", Some(sc_agent_types::ErrorCategory::Config)),
    AgentConfigError::Read { .. } => ("config.read", Some(sc_agent_types::ErrorCategory::Config)),
    AgentConfigError::Json { .. } => ("config.json", Some(sc_agent_types::ErrorCategory::Config)),
    AgentConfigError::Load(_) => ("config.load", Some(sc_agent_types::ErrorCategory::Config)),
    AgentConfigError::Catalog(_) => ("config.catalog", Some(sc_agent_types::ErrorCategory::Config)),
    AgentConfigError::HomeUnknown => ("config.home_unknown", Some(sc_agent_types::ErrorCategory::Config)),
    AgentConfigError::FileTooLarge { .. } => ("config.file_too_large", Some(sc_agent_types::ErrorCategory::Config)),
    AgentConfigError::NotAFile { .. } => ("config.not_a_file", Some(sc_agent_types::ErrorCategory::Config)),
    AgentConfigError::RunnerName { .. } => ("config.runner_name", Some(sc_agent_types::ErrorCategory::Config)),
    AgentConfigError::Invalid { .. } => ("config.invalid", Some(sc_agent_types::ErrorCategory::Config)),
    AgentConfigError::KeyOnPlainHttp { .. } => ("config.key_on_plain_http", Some(sc_agent_types::ErrorCategory::Config)),
} }

impl ErrorId for AgentConfigError {
    fn id(&self) -> &'static str {
        self.catalog_row().0
    }
    fn base_category(&self) -> Option<sc_agent_types::ErrorCategory> {
        self.catalog_row().1
    }

    fn message(&self) -> String {
        match self {
            Self::ProjectRequired { cli } => format!(
                "project/phase is required: set --project, SC_PROJECT, or project in sc-agent.json or {cli}"
            ),
            Self::Layer { source, cause } => format!("{source}: {cause}"),
            Self::Read { file, cause } => format!("{} at $: {cause}", file.display()),
            Self::Json { file, cause } => format!("{} at $: {cause}", file.display()),
            Self::Load(_) => "the sc-agent configuration could not be loaded".to_owned(),
            Self::Catalog(_) => "a configured model conflicts with the model catalog".to_owned(),
            Self::HomeUnknown => {
                "no home directory is known, so ~/.sc-agent cannot be located".to_owned()
            }
            Self::FileTooLarge { file, bytes } => format!(
                "{} is {bytes} bytes, more than the 1 MiB a config file may have",
                file.display()
            ),
            Self::NotAFile { file } => format!(
                "{} is not a regular file (a FIFO, device or directory is never read)",
                file.display()
            ),
            Self::RunnerName { name, path } => {
                format!("{path}: local runner `{name}` is named like a built-in short name")
            }
            Self::Invalid { path, message } => format!("invalid value at {path}: {message}"),
            Self::KeyOnPlainHttp { path, host } => format!(
                "{path}: key_env is set but host {host} is not loopback; local runners use plain http, so the key would leave the machine unencrypted"
            ),
        }
    }

    fn cause_detail(&self) -> Option<ErrorDetail> {
        match self {
            Self::Layer { cause, .. } => Some(cause.detail()),
            Self::Read { cause, .. } => Some(ErrorDetail::foreign("std.io", cause)),
            Self::Json { cause, .. } => Some(ErrorDetail::foreign("serde_json", cause)),
            Self::ProjectRequired { .. } => None,
            Self::Load(inner) => Some(ErrorDetail::foreign(inner.code(), inner)),
            Self::Catalog(inner) => Some(inner.detail()),
            Self::HomeUnknown
            | Self::FileTooLarge { .. }
            | Self::NotAFile { .. }
            | Self::RunnerName { .. }
            | Self::Invalid { .. }
            | Self::KeyOnPlainHttp { .. } => None,
        }
    }

    fn recovery(&self) -> Option<&'static str> {
        match self {
            Self::ProjectRequired { .. } => {
                Some("set the project/phase through one of the named setting places")
            }
            Self::Layer { .. } => {
                Some("fix the value at the reported path in the layer named in the error")
            }
            Self::Read { .. } => Some("make the named configuration file readable"),
            Self::Json { .. } => Some("fix the JSON in the named configuration file"),
            Self::Invalid { .. } => Some("fix the value at the reported configuration path"),
            Self::KeyOnPlainHttp { .. } => Some("use a loopback host or remove key_env"),
            Self::RunnerName { .. } => Some("rename the runner"),
            // sc-config's own advice for its error code (files, JSON, overrides).
            Self::Load(inner) => Some(inner.recovery()),
            Self::Catalog(inner) => Some(inner.recovery().unwrap_or(
                "keep the model under the line its id already has, or choose a different id",
            )),
            Self::HomeUnknown => Some(
                "set HOME (USERPROFILE on Windows), or set log_root in the config and pass an explicit config_dir to load",
            ),
            Self::FileTooLarge { .. } => {
                Some("shrink the config file below 1 MiB; a real config is a few kilobytes")
            }
            Self::NotAFile { .. } => {
                Some("replace the path with a regular JSON file, or remove it")
            }
        }
    }
}

impl fmt::Display for AgentConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.id(), self.message())
    }
}

impl std::error::Error for AgentConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Layer { cause, .. } => Some(cause.as_ref()),
            Self::Read { cause, .. } => Some(cause),
            Self::Json { cause, .. } => Some(cause),
            Self::ProjectRequired { .. } => None,
            Self::Load(inner) => Some(inner),
            Self::Catalog(inner) => Some(inner),
            Self::HomeUnknown
            | Self::FileTooLarge { .. }
            | Self::NotAFile { .. }
            | Self::RunnerName { .. }
            | Self::Invalid { .. }
            | Self::KeyOnPlainHttp { .. } => None,
        }
    }
}

impl From<sc_config::ConfigError> for AgentConfigError {
    fn from(inner: sc_config::ConfigError) -> Self {
        Self::Load(inner)
    }
}

impl From<TypesError> for AgentConfigError {
    fn from(inner: TypesError) -> Self {
        Self::Catalog(inner)
    }
}
