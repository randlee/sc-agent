//! v2 `LoggerBuilder` adapted to [`sc_agent_types::observability::v2::SessionLog`].

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sc_agent_types::observability::v2::{EventError, LogEvent as SessionEvent, SessionLog};
use sc_agent_types::{JevError, RepoRoot};
use sc_observability::v2::{Logger, LoggerBuilder};
use sc_observability::{
    ActionName, Level, LogEvent, LoggerConfig, OBSERVATION_ENVELOPE_VERSION, ProcessIdentity,
    SchemaVersion, ServiceName, TargetCategory, Timestamp, constants,
};
use serde_json::{Map, Value};

/// Environment variable that overrides the default log root.
///
/// `--log-dir` wins over this variable. When both are absent, the log root is
/// `<repo-root>/.sc-ai`.
pub const LOG_ROOT_ENV: &str = "SC_LOG_ROOT";

/// Directory name, relative to the repo root, used when no log root is set.
pub const DEFAULT_LOG_ROOT_DIR: &str = ".sc-ai";

const SERVICE: &str = "sc-ai";

/// Chooses the directory that owns `logs/sc-ai.log.jsonl`.
///
/// `log_dir` is the `--log-dir` flag. When it is absent, [`LOG_ROOT_ENV`] is
/// used if it is set to a non-empty value. Otherwise the directory is
/// `<repo>/.sc-ai`.
#[must_use]
pub fn default_log_root(repo: &RepoRoot, log_dir: Option<&Path>) -> PathBuf {
    if let Some(dir) = log_dir {
        return dir.to_path_buf();
    }
    match std::env::var(LOG_ROOT_ENV) {
        Ok(value) if !value.is_empty() => PathBuf::from(value),
        _ => repo.as_path().join(DEFAULT_LOG_ROOT_DIR),
    }
}

/// Default session-file directory for `repo`.
#[must_use]
pub fn default_sessions_dir(repo: &RepoRoot) -> PathBuf {
    repo.as_path().join(DEFAULT_LOG_ROOT_DIR).join("sessions")
}

/// JSONL path the v2 file sink writes for this service.
#[must_use]
pub fn session_log_file(log_root: &Path) -> PathBuf {
    log_root
        .join(constants::DEFAULT_LOG_DIR_NAME)
        .join(format!("{SERVICE}{}", constants::DEFAULT_LOG_FILE_SUFFIX))
}

/// `sc-observability` v2 logger used as a [`SessionLog`].
///
/// Drop shuts the writer thread down. Call [`Self::flush`] after a successful
/// command so the JSONL file is durable before the process prints stdout.
pub struct ObservabilitySessionLog {
    logger: Logger,
}

impl ObservabilitySessionLog {
    /// Builds a file logger rooted at `log_root`.
    ///
    /// The console sink stays off so stderr remains the error channel.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::ConfigInvalid`] when `log_root` cannot be created
    /// or the v2 logger refuses the configuration. The fix hint names the
    /// directory and does not include secret material.
    pub fn open(log_root: &Path) -> Result<Arc<Self>, JevError> {
        std::fs::create_dir_all(log_root).map_err(|error| JevError::ConfigInvalid {
            detail: format!(
                "Create log directory {} or pass a writable --log-dir ({error}).",
                log_root.display()
            ),
        })?;
        let mut config = LoggerConfig::default_for(service_name(), log_root.to_path_buf());
        config.enable_file_sink = true;
        config.enable_console_sink = false;
        let logger = LoggerBuilder::new(config)
            .map_err(|error| init_error(&error))?
            .build()
            .map_err(|error| init_error(&error))?;
        Ok(Arc::new(Self { logger }))
    }

    /// Flushes admitted events to the JSONL sink.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::ConfigInvalid`] when the sink cannot flush. The
    /// hint tells the caller to check the log root.
    pub fn flush(&self) -> Result<(), JevError> {
        self.logger
            .flush()
            .map_err(|error| JevError::ConfigInvalid {
                detail: format!(
                    "session log flush failed ({}). Check that the log root is writable.",
                    error.diagnostic().message
                ),
            })
    }
}

impl Drop for ObservabilitySessionLog {
    fn drop(&mut self) {
        let _ = self.logger.shutdown();
    }
}

impl SessionLog for ObservabilitySessionLog {
    fn try_log(&self, event: SessionEvent) -> Result<(), EventError> {
        self.logger.try_log(obs_event(event))
    }
}

fn service_name() -> ServiceName {
    ServiceName::new(SERVICE).expect("sc-ai is a valid service name")
}

fn init_error(error: &impl DiagnosticMessage) -> JevError {
    JevError::ConfigInvalid {
        detail: format!(
            "session log could not be started ({}). Choose a writable --log-dir or set {LOG_ROOT_ENV}.",
            error.diagnostic_message()
        ),
    }
}

trait DiagnosticMessage {
    fn diagnostic_message(&self) -> &str;
}

impl DiagnosticMessage for sc_observability::v2::InitError {
    fn diagnostic_message(&self) -> &str {
        &self.diagnostic().message
    }
}

fn obs_event(event: SessionEvent) -> LogEvent {
    let mut fields = Map::new();
    fields.insert(
        "backend".to_owned(),
        Value::String(event.backend.to_string()),
    );
    fields.insert(
        "prompt_sha256".to_owned(),
        Value::String(event.prompt_sha256.as_str().to_owned()),
    );
    fields.insert(
        "operation".to_owned(),
        Value::String(event.operation.clone()),
    );
    fields.insert(
        "session_id".to_owned(),
        Value::String(event.session_id.to_string()),
    );
    LogEvent {
        version: SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION).expect("v1 schema"),
        timestamp: Timestamp::now_utc(),
        level: Level::Info,
        service: service_name(),
        target: TargetCategory::new("jev.session").expect("jev.session is a valid target"),
        action: action_name(&event.operation),
        message: None,
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: None,
        diagnostic: None,
        state_transition: None,
        fields,
    }
}

fn action_name(operation: &str) -> ActionName {
    ActionName::new(operation)
        .unwrap_or_else(|_| ActionName::new("session").expect("session is a valid action name"))
}
