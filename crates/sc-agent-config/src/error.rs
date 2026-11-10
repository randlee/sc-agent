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
    /// sc-config failed: a file is unreadable or not JSON, or an override cannot be applied.
    Load(sc_config::ConfigError),
    /// A `models` entry conflicts with the catalog (a model on two lines).
    Catalog(TypesError),
    /// There is no home directory, so `~/.sc-agent` cannot be located.
    HomeUnknown,
    /// `default.json` or `local.json` is larger than `MAX_CONFIG_FILE_BYTES` (1 MiB).
    FileTooLarge {
        /// The file.
        file: PathBuf,
        /// Its size in bytes.
        bytes: u64,
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

impl ErrorId for AgentConfigError {
    fn id(&self) -> &'static str {
        match self {
            Self::Load(_) => "config.load",
            Self::Catalog(_) => "config.catalog",
            Self::HomeUnknown => "config.home_unknown",
            Self::FileTooLarge { .. } => "config.file_too_large",
            Self::RunnerName { .. } => "config.runner_name",
            Self::Invalid { .. } => "config.invalid",
            Self::KeyOnPlainHttp { .. } => "config.key_on_plain_http",
        }
    }

    fn message(&self) -> String {
        match self {
            Self::Load(_) => "the sc-agent configuration could not be loaded".to_owned(),
            Self::Catalog(_) => "a configured model conflicts with the model catalog".to_owned(),
            Self::HomeUnknown => {
                "no home directory is known, so ~/.sc-agent cannot be located".to_owned()
            }
            Self::FileTooLarge { file, bytes } => format!(
                "{} is {bytes} bytes, more than the 1 MiB a config file may have",
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
            Self::Load(inner) => Some(ErrorDetail::foreign(inner.code(), inner)),
            Self::Catalog(inner) => Some(inner.detail()),
            Self::HomeUnknown
            | Self::FileTooLarge { .. }
            | Self::RunnerName { .. }
            | Self::Invalid { .. }
            | Self::KeyOnPlainHttp { .. } => None,
        }
    }

    fn recovery(&self) -> Option<&'static str> {
        match self {
            Self::Invalid { .. } => Some(
                "fix the value at the reported path in default.json/local.json or its SC_AGENT__ variable",
            ),
            Self::KeyOnPlainHttp { .. } => Some("use a loopback host or remove key_env"),
            Self::RunnerName { .. } => Some("rename the runner"),
            // sc-config's own advice for its error code (files, JSON, overrides).
            Self::Load(inner) => Some(inner.recovery()),
            Self::Catalog(inner) => Some(inner.recovery().unwrap_or(
                "keep the model under the line its id already has, or choose a different id",
            )),
            Self::HomeUnknown => Some(
                "set HOME, or set log_root in the config and pass an explicit config_dir to load",
            ),
            Self::FileTooLarge { .. } => {
                Some("shrink the config file below 1 MiB; a real config is a few kilobytes")
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
            Self::Load(inner) => Some(inner),
            Self::Catalog(inner) => Some(inner),
            Self::HomeUnknown
            | Self::FileTooLarge { .. }
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
