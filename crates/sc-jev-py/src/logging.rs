//! `sc-observability` v2 logger adapted to [`SharedSessionLog`].
//!
//! Construction uses [`sc_observability::v2::LoggerBuilder`] (ADR-SAG-0013).
//! Path precedence is ADR-SAG-0021. The JSONL file uses the crate's built-in
//! file sink. This module does not write JSONL itself.
//!
//! One logger (one file writer) exists per canonical log root in this
//! process. [`open_session_log`] reuses a live one from a process-wide
//! registry of weak handles, so repeated `run_check` calls in one interpreter
//! share the writer. The last handle to drop flushes and shuts it down.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, Weak};

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

/// Target category recorded on every Python session-log line.
const TARGET: &str = "jev.session";

/// Action recorded when an operation name is not a valid action name.
const FALLBACK_ACTION: &str = "event";

/// Owns one running logger. Dropping it flushes and shuts the writer down.
///
/// Shutdown runs exactly once, when the last [`Arc`] goes away, so there is
/// no reference-count race between concurrent drops.
struct LoggerInner(Logger);

impl Drop for LoggerInner {
    fn drop(&mut self) {
        let _ = self.0.flush();
        let _ = self.0.shutdown();
    }
}

/// Shared logger handle; clones share one writer.
#[derive(Clone)]
pub(crate) struct LoggerGuard {
    inner: Arc<LoggerInner>,
}

impl LoggerGuard {
    fn log(&self, event: LogEvent) -> Result<(), EventError> {
        self.inner.0.log(event)
    }

    pub(crate) fn flush(&self) {
        let _ = self.inner.0.flush();
    }

    /// Whether `self` and `other` write through the same logger.
    #[cfg(test)]
    pub(crate) fn shares_writer_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

/// Live loggers keyed by canonical log root (composition-root state).
type Registry = Mutex<HashMap<PathBuf, Weak<LoggerInner>>>;

fn registry() -> &'static Registry {
    static LOGGERS: OnceLock<Registry> = OnceLock::new();
    LOGGERS.get_or_init(|| Mutex::new(HashMap::new()))
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

/// Returns the file logger for `log_root` and its [`SharedSessionLog`] adapter.
///
/// A logger already running for the same canonical root is reused; otherwise
/// one is started. Console logging stays off so JSONL is the retained record.
/// Bearer tokens in event text are redacted by the logger. This adapter never
/// puts an API key in the event.
///
/// # Errors
///
/// Returns [`JevError::session_store_io`] (`CONFIG_INVALID`) when `log_root`
/// cannot be created or resolved, or the logger cannot start. The fix hint
/// names `log_root`.
pub(crate) fn open_session_log(
    log_root: &Path,
) -> Result<(LoggerGuard, SharedSessionLog), JevError> {
    let guard = shared_logger(log_root)?;
    let shared: SharedSessionLog = Arc::new(ObservabilitySessionLog {
        logger: guard.clone(),
    });
    Ok((guard, shared))
}

fn shared_logger(log_root: &Path) -> Result<LoggerGuard, JevError> {
    std::fs::create_dir_all(log_root).map_err(|error| {
        JevError::session_store_io(&format!(
            "Create log directory {} or set {LOG_DIR_ENV} to a writable directory ({error}).",
            log_root.display()
        ))
    })?;
    let key = std::fs::canonicalize(log_root).map_err(|error| {
        JevError::session_store_io(&format!(
            "Resolve log directory {} ({error}). Choose a readable log directory and retry.",
            log_root.display()
        ))
    })?;
    let mut live = registry().lock().unwrap_or_else(PoisonError::into_inner);
    live.retain(|_, weak| weak.strong_count() > 0);
    if let Some(inner) = live.get(&key).and_then(Weak::upgrade) {
        return Ok(LoggerGuard { inner });
    }
    let inner = Arc::new(LoggerInner(start_logger(&key)?));
    live.insert(key, Arc::downgrade(&inner));
    Ok(LoggerGuard { inner })
}

fn start_logger(log_root: &Path) -> Result<Logger, JevError> {
    let mut config = LoggerConfig::default_for(service_name(), log_root.to_path_buf());
    config.enable_console_sink = false;
    config.enable_file_sink = true;
    LoggerBuilder::new(config)
        .and_then(LoggerBuilder::build)
        .map_err(|error| {
            JevError::session_store_io(&format!(
                "Start the session logger at {} ({error}). Choose a writable log directory and retry.",
                log_root.display()
            ))
        })
}

/// Default log root when no argument or environment variable is set.
///
/// `~/.sc-agent/log`, or `%USERPROFILE%\.sc-agent\log` when `HOME` is unset.
/// When both are unset, the path is the process temp directory plus
/// `sc-agent/log` (ADR-SAG-0021).
#[must_use]
pub(crate) fn default_log_root() -> PathBuf {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
    default_log_root_from(home, &std::env::temp_dir())
}

/// Pure form of [`default_log_root`] over an injected home and temp dir.
///
/// An empty `home` counts as unset.
#[must_use]
pub(crate) fn default_log_root_from(home: Option<OsString>, tmp: &Path) -> PathBuf {
    match home.filter(|home| !home.is_empty()) {
        Some(home) => PathBuf::from(home).join(".sc-agent").join("log"),
        None => tmp.join("sc-agent").join("log"),
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

// The `expect`s below run on compile-time constants. The unit test
// `observability_constants_are_valid` proves each one holds.

fn service_name() -> ServiceName {
    ServiceName::new(SERVICE_NAME).expect("SERVICE_NAME is a valid service name")
}

fn target() -> TargetCategory {
    TargetCategory::new(TARGET).expect("TARGET is a valid target category")
}

fn action_name(operation: &str) -> ActionName {
    ActionName::new(operation).unwrap_or_else(|_| {
        ActionName::new(FALLBACK_ACTION).expect("FALLBACK_ACTION is a valid action name")
    })
}

fn schema_version() -> SchemaVersion {
    SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION)
        .expect("OBSERVATION_ENVELOPE_VERSION is a valid schema version")
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
        version: schema_version(),
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
    use tempfile::TempDir;

    fn event(session_id: SessionId, digest: &Sha256Digest, operation: &str) -> SessionEvent {
        SessionEvent {
            session_id,
            backend: BackendId::AnthropicHaiku,
            prompt_sha256: digest.clone(),
            operation: operation.to_owned(),
        }
    }

    #[test]
    fn observability_constants_are_valid() {
        assert!(ServiceName::new(SERVICE_NAME).is_ok());
        assert!(TargetCategory::new(TARGET).is_ok());
        assert!(ActionName::new(FALLBACK_ACTION).is_ok());
        assert!(SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION).is_ok());
        // An operation that is not an action name falls back instead of panicking.
        let _ = action_name("not a valid action name!");
    }

    #[test]
    fn default_log_root_uses_home_when_set() {
        let tmp = Path::new("tmp-root");
        assert_eq!(
            default_log_root_from(Some(OsString::from("home-root")), tmp),
            Path::new("home-root").join(".sc-agent").join("log")
        );
    }

    #[test]
    fn default_log_root_falls_back_to_temp_when_home_is_unset_or_empty() {
        let tmp = Path::new("tmp-root");
        let expected = tmp.join("sc-agent").join("log");
        assert_eq!(default_log_root_from(None, tmp), expected);
        assert_eq!(default_log_root_from(Some(OsString::new()), tmp), expected);
    }

    #[test]
    fn explicit_log_root_wins() {
        let temp_dir = TempDir::new().expect("temp");
        let explicit = temp_dir.path().join("sc-jev-py-explicit");
        let sessions = temp_dir.path().join("sessions");
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
        let root = TempDir::new().expect("temp");
        let (guard, log) = open_session_log(root.path()).expect("logger");
        let digest = Sha256Digest::of(b"payload");
        let session_id = SessionId::new();
        log.try_log(event(session_id, &digest, "judge"))
            .expect("log");
        guard.flush();
        let text = std::fs::read_to_string(log_file(root.path())).expect("jsonl");
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
    }

    #[test]
    fn one_writer_per_log_root_while_any_handle_lives() {
        let root = TempDir::new().expect("temp");
        let (first, first_log) = open_session_log(root.path()).expect("first");
        // A different spelling of the same directory maps to the same key.
        let (second, second_log) = open_session_log(&root.path().join(".")).expect("second");
        assert!(first.shares_writer_with(&second));
        let digest = Sha256Digest::of(b"payload");
        first_log
            .try_log(event(SessionId::new(), &digest, "judge"))
            .expect("first line");
        second_log
            .try_log(event(SessionId::new(), &digest, "ask"))
            .expect("second line");
        second.flush();
        let text = std::fs::read_to_string(log_file(root.path())).expect("jsonl");
        assert_eq!(text.lines().filter(|line| !line.is_empty()).count(), 2);

        let other = TempDir::new().expect("other");
        let (third, _third_log) = open_session_log(other.path()).expect("third");
        assert!(!third.shares_writer_with(&first));

        drop((first, first_log, second, second_log));
        let (fresh, _fresh_log) = open_session_log(root.path()).expect("fresh");
        assert!(
            fresh.inner.0.flush().is_ok(),
            "a fresh logger starts after the last drop"
        );
    }
}
