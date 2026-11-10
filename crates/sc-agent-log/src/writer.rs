//! [`Writer`]: one running sc-observability logger with its lost-row accounting.
//!
//! Both [`GeneralLogger`](crate::GeneralLogger) and [`SessionLogger`](crate::SessionLogger) wrap
//! a `Writer`. It builds the v2 [`LogEvent`], admits it (blocking or not), and counts every row
//! that was refused at write or that the sc-observability writer thread could not write, so a
//! caller that cannot act on a log failure still loses nothing silently: [`Writer::flush`]
//! reports each such row once as [`LogError::RowsLost`].

use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use sc_agent_types::{ErrorDetail, ErrorId, SessionId};
use sc_observability::v2::{LogEvent, Logger, LoggerBuilder, LoggerConfig, SinkRegistration};
use sc_observability::{
    ActionName, Level, OBSERVATION_ENVELOPE_VERSION, ProcessIdentity, SchemaVersion, ServiceName,
    TargetCategory, Timestamp,
};
use sc_observability_types::CorrelationId;
use serde_json::{Map, Value};

use crate::LogError;

/// Whether a write may wait for a free queue slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// `Logger::log`: waits while the writer queue is full.
    Blocking,
    /// `Logger::try_log`: a full queue is an error, counted as a lost row (RSH-010).
    NonBlocking,
}

/// Owns or borrows one running logger.
///
/// Dropping an owned writer flushes and shuts the logger down; an injected logger is never
/// flushed-on-drop or shut down here (REQ-SAG-0008).
pub(crate) struct Writer {
    logger: Arc<Logger>,
    owned: bool,
    /// The logger's service; every event carries it (sc-observability refuses another).
    service: ServiceName,
    /// All lost-row state under one lock (RBP-F015).
    accounting: Mutex<Accounting>,
    /// `"general"`, `"session"` or `"memory"`, for errors.
    log: &'static str,
}

/// All lost-row state under one lock (RBP-F015): flush reads and resets it in one critical section.
///
/// Mutated only by a counter add or one assignment per critical section, and no call leaves the
/// crate while the lock is held, so after a panic elsewhere the state is still valid and
/// `PoisonError::into_inner` is safe.
#[derive(Debug, Default)]
struct Accounting {
    /// Rows write/try_write returned Err for since the last report.
    refused: u64,
    /// sc-observability's lost-row total already reported.
    writer_lost_seen: u64,
    /// The latest refusal, taken by the report.
    last: Option<ErrorDetail>,
}

impl Writer {
    /// A writer that owns `logger`: Drop flushes and shuts it down.
    pub(crate) fn owned(logger: Logger, log: &'static str) -> Self {
        Self::new(Arc::new(logger), true, log)
    }

    /// A writer over an injected `logger`, never shut down here.
    pub(crate) fn injected(logger: Arc<Logger>, log: &'static str) -> Self {
        Self::new(logger, false, log)
    }

    fn new(logger: Arc<Logger>, owned: bool, log: &'static str) -> Self {
        // Losses before this writer existed are not this writer's to report.
        let writer_lost_seen = logger.health().dropped_events_total;
        Self {
            service: logger.service_name().clone(),
            logger,
            owned,
            accounting: Mutex::new(Accounting {
                writer_lost_seen,
                ..Accounting::default()
            }),
            log,
        }
    }

    /// Builds and admits one event; any Err is counted before it is returned.
    pub(crate) fn write(
        &self,
        mode: Mode,
        level: Level,
        target: &'static str,
        action: &'static str,
        correlation: Option<SessionId>,
        fields: Result<Map<String, Value>, LogError>,
    ) -> Result<(), LogError> {
        fields
            .and_then(|fields| self.event(level, target, action, correlation, fields))
            .and_then(|event| self.admit(event, mode))
            .map_err(|error| self.count(error))
    }

    /// The v2 `LogEvent` for one row; `correlation_id` is the session id, if any.
    fn event(
        &self,
        level: Level,
        target: &'static str,
        action: &'static str,
        correlation: Option<SessionId>,
        fields: Map<String, Value>,
    ) -> Result<LogEvent, LogError> {
        let correlation_id = correlation
            .map(|id| {
                let text = id.to_string();
                CorrelationId::new(text.as_str())
                    .map_err(|error| LogError::invalid_name("correlation_id", &text, &error))
            })
            .transpose()?;
        Ok(LogEvent {
            version: SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION).map_err(|error| {
                LogError::invalid_name("version", OBSERVATION_ENVELOPE_VERSION, &error)
            })?,
            timestamp: Timestamp::now_utc(),
            level,
            service: self.service.clone(),
            target: TargetCategory::new(target)
                .map_err(|error| LogError::invalid_name("target", target, &error))?,
            action: ActionName::new(action)
                .map_err(|error| LogError::invalid_name("action", action, &error))?,
            message: None,
            identity: ProcessIdentity::default(),
            trace: None,
            request_id: None,
            correlation_id,
            outcome: None,
            diagnostic: None,
            state_transition: None,
            fields,
        })
    }

    fn admit(&self, event: LogEvent, mode: Mode) -> Result<(), LogError> {
        let admitted = match mode {
            Mode::Blocking => self.logger.log(event),
            Mode::NonBlocking => self.logger.try_log(event),
        };
        admitted.map_err(|source| LogError::Rejected {
            log: self.log,
            source,
        })
    }

    /// Counts one refused row and keeps it as the latest failure.
    fn count(&self, error: LogError) -> LogError {
        let detail = error.detail();
        let mut accounting = self.accounting();
        accounting.refused = accounting.refused.saturating_add(1);
        accounting.last = Some(detail);
        drop(accounting);
        error
    }

    fn accounting(&self) -> MutexGuard<'_, Accounting> {
        // Poison: see `Accounting`; the state is a valid count after any panic.
        self.accounting
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Flush barrier; then [`LogError::RowsLost`] if any row since the last report was
    /// refused or not written. Each lost row is reported once.
    pub(crate) fn flush(&self) -> Result<(), LogError> {
        self.logger.flush().map_err(|source| LogError::Flush {
            log: self.log,
            source,
        })?;
        // Read health before the lock: no call leaves the crate while it is held.
        let health = self.logger.health();
        let total = health.dropped_events_total;
        let mut accounting = self.accounting();
        let newly_lost = total.saturating_sub(accounting.writer_lost_seen);
        accounting.writer_lost_seen = accounting.writer_lost_seen.max(total);
        let count = accounting.refused.saturating_add(newly_lost);
        accounting.refused = 0;
        let refused_last = accounting.last.take();
        drop(accounting);
        if count == 0 {
            return Ok(());
        }
        // A writer loss is observed at this flush, after any refusal admitted before it.
        let writer_last = (newly_lost > 0)
            .then(|| health.last_error.or(health.last_writer_error))
            .flatten()
            .map(|summary| {
                let id = summary
                    .code
                    .as_ref()
                    .map_or("sc_observability.writer", |code| code.as_str());
                ErrorDetail::new(id, summary.message, None)
            });
        Err(LogError::RowsLost {
            log: self.log,
            count,
            last: writer_last.or(refused_last),
        })
    }
}

impl fmt::Debug for Writer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Writer")
            .field("log", &self.log)
            .field("service", &self.service)
            .field("owned", &self.owned)
            .finish_non_exhaustive()
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        if !self.owned {
            return;
        }
        // Best effort: the process is done with this writer and a failure here has no caller
        // left to report to.
        let _ = self.logger.flush();
        let _ = self.logger.shutdown();
    }
}

/// The logger configuration for `service` at `log_root`: console off, built-in file sink as given.
pub(crate) fn logger_config(
    service: &'static str,
    log_root: &Path,
    file_sink: bool,
) -> Result<LoggerConfig, LogError> {
    let name = ServiceName::new(service)
        .map_err(|error| LogError::invalid_name("service", service, &error))?;
    let mut config = LoggerConfig::default_for(name, log_root.to_path_buf());
    config.enable_console_sink = false;
    config.enable_file_sink = file_sink;
    Ok(config)
}

/// Starts a logger on `config`, with `extra` registered beside the built-in sinks.
pub(crate) fn start_logger(
    config: LoggerConfig,
    log: &'static str,
    extra: Option<SinkRegistration>,
) -> Result<Logger, LogError> {
    let mut builder =
        LoggerBuilder::new(config).map_err(|source| LogError::Start { log, source })?;
    if let Some(registration) = extra {
        builder.register_sink(registration);
    }
    builder
        .build()
        .map_err(|source| LogError::Start { log, source })
}
