//! [`GeneralLogger`]: the general (diagnostic) log, one file per CLI (REQ-SAG-0003).

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sc_observability::Level;
use sc_observability::constants::{DEFAULT_LOG_DIR_NAME, DEFAULT_LOG_FILE_SUFFIX};
use sc_observability::v2::{JsonlFileSink, Logger, LoggerConfig};
use serde_json::{Map, Value};

use crate::writer::{Ledger, Mode, Writer, logger_config, start_logger};
use crate::{LogContext, LogError, LogKind};

/// The kind of the general logger in errors.
const LOG: LogKind = LogKind::General;

/// The general (diagnostic) log. Clones share one writer.
///
/// The default writes sc-observability's built-in file `<log_root>/logs/<service>.log.jsonl`,
/// one file per CLI (`decide`, `sc-agent-py`), under sc-observability's default
/// `RetainedLogPolicy` and level; the file's retention is sc-observability's, not this crate's
/// (REQ-SAG-0003). An injected logger is used as given and never shut down here (REQ-SAG-0008).
///
/// # Examples
///
/// ```
/// use sc_agent_log::{GeneralLogger, Level, LogContext};
/// use sc_agent_types::SessionId;
///
/// let root = tempfile::tempdir()?;
/// let log = GeneralLogger::open_default(root.path(), "decide")?;
/// let mut fields = serde_json::Map::new();
/// fields.insert("elapsed_ms".into(), 12.into());
/// let ctx = LogContext::request(SessionId::new(), Some(0));
/// log.write(Level::Info, "sc_agent_http", "http.response", ctx, fields)?;
/// log.flush()?;
/// assert!(root.path().join("logs").join("decide.log.jsonl").exists());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone)]
pub struct GeneralLogger {
    writer: Arc<Writer>,
}

impl GeneralLogger {
    /// Written first in an event that has a context; a payload may not use them.
    pub const CONTEXT_KEYS: [&'static str; 2] = ["session_id", "position"];

    /// The default general log of CLI `service`: `<log_root>/logs/<service>.log.jsonl`.
    ///
    /// sc-observability's built-in file sink, its default `RetainedLogPolicy` and level, console
    /// off. One retention pass runs now, so a short-lived CLI that never reaches
    /// sc-observability's first periodic pass (60 s after start) still applies the policy.
    ///
    /// # Errors
    ///
    /// [`LogError::InvalidName`] when `service` is not a valid sc-observability service name,
    /// [`LogError::OpenSink`] when the startup retention pass fails (for example, a log
    /// directory that cannot be written), [`LogError::Start`] when the logger cannot start.
    pub fn open_default(log_root: &Path, service: &'static str) -> Result<Self, LogError> {
        let config = logger_config(service, log_root, true)?;
        prepare_log_file(&config)?;
        let logger = start_logger(config, LOG, None)?;
        Ok(Self {
            writer: Arc::new(Writer::owned(logger, LOG)),
        })
    }

    /// A general logger writing to the injected `logger`; it is never flushed-on-drop or shut
    /// down here.
    #[must_use]
    pub fn injected(logger: Arc<Logger>) -> Self {
        Self::injected_in(logger, None)
    }

    /// A general logger on `logger` sharing `ledger`, the lost-row accounting of other writers
    /// on the same logger.
    pub(crate) fn injected_in(logger: Arc<Logger>, ledger: Option<Arc<Ledger>>) -> Self {
        Self {
            writer: Arc::new(Writer::injected(logger, ledger, LOG)),
        }
    }

    /// Writes one event; blocks while the sc-observability writer queue is full.
    ///
    /// `target` names the calling crate (`"sc_agent_http"`, `"sc_decision_runner"`); `action`
    /// names the event, e.g. `"http.busy"`. `ctx`'s session id becomes the event's
    /// correlation id and the first field `session_id`, its position the field `position`;
    /// [`LogContext::NONE`] adds neither. Callers never pass a body, header value or key.
    ///
    /// The wait for a queue slot is bounded by the writer thread's disk throughput, not a timer.
    /// Async callers use [`Self::try_write`], or run `write` on a blocking thread (see the
    /// crate docs).
    ///
    /// # Errors
    ///
    /// [`LogError::ReservedField`] when `fields` uses a [`Self::CONTEXT_KEYS`] key,
    /// [`LogError::RowTooLarge`] when the fields are over [`MAX_ROW_BYTES`](crate::MAX_ROW_BYTES),
    /// [`LogError::InvalidName`] for a bad `target` or `action`, [`LogError::Rejected`] when
    /// sc-observability refuses the event. Every Err is also counted and reported once by
    /// [`Self::flush`], so a caller that must not act on it loses nothing.
    pub fn write(
        &self,
        level: Level,
        target: &'static str,
        action: &'static str,
        ctx: LogContext,
        fields: Map<String, Value>,
    ) -> Result<(), LogError> {
        self.writer.write(
            Mode::Blocking,
            level,
            target,
            action,
            ctx.session_id,
            context_fields(ctx, fields),
        )
    }

    /// Same event as [`Self::write`], but never blocks (`Logger::try_log`).
    ///
    /// For paths where a stalled disk must not delay a result (RSH-010).
    ///
    /// # Errors
    ///
    /// As [`Self::write`]; a full queue is a [`LogError::Rejected`], counted as a lost row.
    pub fn try_write(
        &self,
        level: Level,
        target: &'static str,
        action: &'static str,
        ctx: LogContext,
        fields: Map<String, Value>,
    ) -> Result<(), LogError> {
        self.writer.write(
            Mode::NonBlocking,
            level,
            target,
            action,
            ctx.session_id,
            context_fields(ctx, fields),
        )
    }

    /// Flush barrier; then reports rows refused or not written since the last report.
    ///
    /// # Errors
    ///
    /// [`LogError::Flush`] when the barrier fails; [`LogError::RowsLost`] with the count and the
    /// latest failure when any row since the last report was lost (each reported once).
    pub fn flush(&self) -> Result<(), LogError> {
        self.writer.flush()
    }

    /// Flushes, shuts the default logger down, and returns what was lost (RSH-001).
    ///
    /// Use this instead of dropping the logger where lost rows must be returned to the caller.
    /// When other clones of this logger are alive, it only flushes and reports; the last clone
    /// to go shuts the logger down. An injected logger is flushed, never shut down. Dropping a
    /// logger without `close` does the same work best-effort and writes one last-resort line to
    /// stderr if rows were lost.
    ///
    /// # Errors
    ///
    /// As [`Self::flush`]: [`LogError::Flush`] or [`LogError::RowsLost`].
    pub fn close(self) -> Result<(), LogError> {
        match Arc::try_unwrap(self.writer) {
            Ok(writer) => writer.close(),
            Err(shared) => shared.flush(),
        }
    }
}

impl fmt::Debug for GeneralLogger {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneralLogger")
            .field("writer", &self.writer)
            .finish()
    }
}

/// Context fields first (`session_id`, `position`), then `fields` in its order.
fn context_fields(
    ctx: LogContext,
    fields: Map<String, Value>,
) -> Result<Map<String, Value>, LogError> {
    if let Some(key) = fields
        .keys()
        .find(|key| GeneralLogger::CONTEXT_KEYS.contains(&key.as_str()))
    {
        return Err(LogError::ReservedField { key: key.clone() });
    }
    if ctx == LogContext::NONE {
        return Ok(fields);
    }
    let mut row = Map::with_capacity(fields.len() + 2);
    if let Some(session_id) = ctx.session_id {
        row.insert(
            "session_id".to_owned(),
            Value::String(session_id.to_string()),
        );
    }
    if let Some(position) = ctx.position {
        row.insert("position".to_owned(), Value::from(position));
    }
    row.extend(fields);
    Ok(row)
}

/// `<log_root>/logs/<service>.log.jsonl`, sc-observability's built-in file path.
fn active_path(config: &LoggerConfig) -> PathBuf {
    config.log_root.join(DEFAULT_LOG_DIR_NAME).join(format!(
        "{}{DEFAULT_LOG_FILE_SUFFIX}",
        config.service_name.as_str()
    ))
}

/// One startup retention pass on the active file, under `config`'s policy.
///
/// sc-observability runs its first periodic pass only 60 s after the logger starts, so a
/// short-lived CLI would otherwise never apply the policy. This is its public startup pass
/// (`JsonlFileSink::open`); the policy and the work are sc-observability's.
fn prepare_log_file(config: &LoggerConfig) -> Result<(), LogError> {
    let active = active_path(config);
    JsonlFileSink::open(active.clone(), config.retained_log_policy)
        .map(drop)
        .map_err(|source| LogError::OpenSink {
            path: active,
            source,
        })
}
