//! `sc-observability` v2 logger adapted to [`SharedSessionLog`].
//!
//! Construction uses [`sc_observability::v2::LoggerBuilder`] (ADR-SAG-0013).
//! Path precedence is ADR-SAG-0021. The JSONL file uses the crate's built-in
//! file sink. This module does not write JSONL itself.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sc_agent_types::JevError;
use sc_agent_types::observability::v2::{
    EventError, LogEvent as SessionEvent, SessionLog, SharedSessionLog,
};
use sc_observability::v2::{Logger, LoggerBuilder};
use sc_observability::{
    ActionName, Level, LogEvent, LoggerConfig, OBSERVATION_ENVELOPE_VERSION, ProcessIdentity,
    SchemaVersion, ServiceName, TargetCategory, Timestamp,
};

/// Process environment that overrides the default log root.
pub(crate) const LOG_DIR_ENV: &str = "SC_AGENT_LOG_DIR";

/// `sc-observability` log-root variable, used when [`LOG_DIR_ENV`] is unset.
pub(crate) const OBS_LOG_ROOT_ENV: &str = "SC_LOG_ROOT";

/// Process environment that overrides the default session directory.
pub(crate) const SESSION_DIR_ENV: &str = "SC_AGENT_SESSION_DIR";

/// Service name recorded on every Python session-log line.
pub(crate) const SERVICE_NAME: &str = "sc-jev-py";

/// Shared logger handle. The last clone flushes and shuts the writer down.
#[derive(Clone)]
pub(crate) struct LoggerGuard {
    logger: Arc<Logger>,
}

impl LoggerGuard {
    fn log(&self, event: LogEvent) -> Result<(), EventError> {
        self.logger.log(event)
    }

    pub(crate) fn flush(&self) {
        let _ = self.logger.flush();
    }
}

impl Drop for LoggerGuard {
    fn drop(&mut self) {
        if Arc::strong_count(&self.logger) == 1 {
            let _ = self.logger.flush();
            let _ = self.logger.shutdown();
        }
    }
}

struct ObservabilitySessionLog {
    logger: LoggerGuard,
}

impl SessionLog for ObservabilitySessionLog {
    fn try_log(&self, event: SessionEvent) -> Result<(), EventError> {
        self.logger.log(to_obs_event(&event))
    }
}

impl std::fmt::Debug for ObservabilitySessionLog {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ObservabilitySessionLog")
            .finish_non_exhaustive()
    }
}

/// Starts a file logger at `log_root` and the [`SharedSessionLog`] adapter.
///
/// Console logging stays off so JSONL is the retained record. Bearer tokens
/// in event text are redacted by the logger. This adapter never puts an API
/// key in the event.
///
/// # Errors
///
/// Returns [`JevError::session_store_io`] (`CONFIG_INVALID`) when `log_root`
/// cannot be created or the logger cannot start. The fix hint names `log_root`.
pub(crate) fn open_session_log(
    log_root: &Path,
) -> Result<(LoggerGuard, SharedSessionLog), JevError> {
    std::fs::create_dir_all(log_root).map_err(|error| {
        JevError::session_store_io(&format!(
            "Create log directory {} or set {LOG_DIR_ENV} to a writable directory ({error}).",
            log_root.display()
        ))
    })?;
    let mut config = LoggerConfig::default_for(service_name(), log_root.to_path_buf());
    config.enable_console_sink = false;
    config.enable_file_sink = true;
    let logger = LoggerBuilder::new(config)
        .and_then(LoggerBuilder::build)
        .map_err(|error| {
            JevError::session_store_io(&format!(
                "Start the session logger at {} ({error}). Choose a writable log directory and retry.",
                log_root.display()
            ))
        })?;
    let guard = LoggerGuard {
        logger: Arc::new(logger),
    };
    let shared: SharedSessionLog = Arc::new(ObservabilitySessionLog {
        logger: guard.clone(),
    });
    Ok((guard, shared))
}

/// Default log root when no argument or environment variable is set.
///
/// `~/.sc-agent/log`, or `%USERPROFILE%\.sc-agent\log` when `HOME` is unset.
/// When both are unset, the path is the process temp directory plus
/// `sc-agent/log` (ADR-SAG-0021).
#[must_use]
pub(crate) fn default_log_root() -> PathBuf {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
    match home {
        Some(home) => PathBuf::from(home).join(".sc-agent").join("log"),
        None => std::env::temp_dir().join("sc-agent").join("log"),
    }
}

/// Resolves the log root (ADR-SAG-0021).
///
/// `explicit` wins, then [`LOG_DIR_ENV`], then [`OBS_LOG_ROOT_ENV`], then
/// [`default_log_root`]. An empty path or environment value is absent.
#[must_use]
pub(crate) fn resolve_log_root(explicit: Option<&Path>) -> PathBuf {
    if let Some(path) = non_empty(explicit) {
        return path.to_path_buf();
    }
    if let Some(path) = env_path(LOG_DIR_ENV) {
        return path;
    }
    if let Some(path) = env_path(OBS_LOG_ROOT_ENV) {
        return path;
    }
    default_log_root()
}

/// Resolves the session directory (ADR-SAG-0021).
///
/// `explicit` wins, then [`SESSION_DIR_ENV`], then `{log_root}/sessions`.
/// An empty path or environment value is absent.
#[must_use]
pub(crate) fn resolve_session_dir(explicit: Option<&Path>, log_root: &Path) -> PathBuf {
    if let Some(path) = non_empty(explicit) {
        return path.to_path_buf();
    }
    if let Some(path) = env_path(SESSION_DIR_ENV) {
        return path;
    }
    log_root.join("sessions")
}

/// Active JSONL path for `log_root`.
#[must_use]
pub(crate) fn log_file(log_root: &Path) -> PathBuf {
    log_root
        .join("logs")
        .join(format!("{SERVICE_NAME}.log.jsonl"))
}

fn non_empty(path: Option<&Path>) -> Option<&Path> {
    path.filter(|path| !path.as_os_str().is_empty())
}

fn env_path(key: &str) -> Option<PathBuf> {
    let value = std::env::var_os(key)?;
    if value.is_empty() {
        None
    } else {
        Some(PathBuf::from(value))
    }
}

fn service_name() -> ServiceName {
    ServiceName::new(SERVICE_NAME).expect("sc-jev-py is a valid service name")
}

fn target() -> TargetCategory {
    TargetCategory::new("jev.session").expect("jev.session is a valid target")
}

fn action_name(operation: &str) -> ActionName {
    ActionName::new(operation)
        .unwrap_or_else(|_| ActionName::new("event").expect("event is a valid action name"))
}

fn to_obs_event(event: &SessionEvent) -> LogEvent {
    let mut fields = serde_json::Map::new();
    fields.insert(
        "session_id".to_owned(),
        serde_json::Value::String(event.session_id.to_string()),
    );
    fields.insert(
        "backend".to_owned(),
        serde_json::Value::String(event.backend.to_string()),
    );
    fields.insert(
        "prompt_sha256".to_owned(),
        serde_json::Value::String(event.prompt_sha256.as_str().to_owned()),
    );
    fields.insert(
        "operation".to_owned(),
        serde_json::Value::String(event.operation.clone()),
    );
    LogEvent {
        version: SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION).expect("v1 is a valid schema"),
        timestamp: Timestamp::now_utc(),
        level: Level::Info,
        service: service_name(),
        target: target(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use sc_agent_types::{BackendId, SessionId, Sha256Digest};

    #[test]
    fn default_log_root_is_under_sc_agent() {
        let path = default_log_root();
        let parts: Vec<_> = path.components().collect();
        assert!(parts.len() >= 2);
        let end: Vec<String> = parts
            .iter()
            .rev()
            .take(2)
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect();
        assert_eq!(end, ["log", ".sc-agent"]);
    }

    #[test]
    fn explicit_log_root_wins() {
        let temp_dir = std::env::temp_dir();
        let explicit = temp_dir.join("sc-jev-py-explicit");
        let sessions = temp_dir.join("sessions");
        assert_eq!(resolve_log_root(Some(&explicit)), explicit);
        assert_eq!(resolve_session_dir(Some(&sessions), &explicit), sessions);
        assert_eq!(
            resolve_session_dir(None, &explicit),
            explicit.join("sessions")
        );
        assert_eq!(
            log_file(&explicit),
            explicit.join("logs").join("sc-jev-py.log.jsonl")
        );
    }

    #[test]
    fn adapter_writes_one_line_without_secret_fields() {
        let root = std::env::temp_dir().join(format!(
            "sc-jev-py-log-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let (guard, log) = open_session_log(&root).expect("logger");
        let digest = Sha256Digest::of(b"payload");
        let session_id = SessionId::new();
        log.try_log(SessionEvent {
            session_id,
            backend: BackendId::AnthropicHaiku,
            prompt_sha256: digest.clone(),
            operation: "judge".to_owned(),
        })
        .expect("log");
        guard.flush();
        let text = std::fs::read_to_string(log_file(&root)).expect("jsonl");
        let lines: Vec<_> = text.lines().filter(|line| !line.is_empty()).collect();
        assert_eq!(lines.len(), 1, "{text}");
        assert!(lines[0].contains(digest.as_str()));
        assert!(lines[0].contains("anthropic_haiku"));
        assert!(lines[0].contains("judge"));
        assert!(lines[0].contains(&session_id.to_string()));
        let lower = text.to_ascii_lowercase();
        for forbidden in ["api_key", "authorization", "bearer ", "password"] {
            assert!(!lower.contains(forbidden), "{forbidden} in {text}");
        }
        drop(log);
        drop(guard);
        let _ = std::fs::remove_dir_all(root);
    }
}
