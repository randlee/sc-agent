//! [`Writer`]: one running sc-observability logger with its lost-row accounting.
//!
//! Both [`GeneralLogger`](crate::GeneralLogger) and [`SessionLogger`](crate::SessionLogger) wrap
//! a `Writer`. It builds the v2 [`LogEvent`], admits it (blocking or not), and counts every row
//! that was refused at write or that the sc-observability writer thread could not write, so a
//! caller that cannot act on a log failure still loses nothing silently: [`Writer::flush`]
//! reports each such row once as [`LogError::RowsLost`], and an owned writer dropped without
//! [`Writer::close`] writes one last-resort line to stderr for anything still unreported.
//!
//! The accounting lives in a [`Ledger`], one per `Logger`: writers that share a `Logger` (built
//! by [`Loggers::open`](crate::Loggers::open) or `MemoryLog`) share its ledger, so a row the
//! `Logger` lost is reported by exactly one flush.

use std::fmt;
use std::io::Write as _;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use sc_agent_types::{ErrorDetail, ErrorId, SessionId};
use sc_observability::v2::{
    LogEvent, Logger, LoggerBuilder, LoggerConfig, ShutdownError, SinkRegistration,
};
use sc_observability::{
    ActionName, Level, OBSERVATION_ENVELOPE_VERSION, ProcessIdentity, SchemaVersion, ServiceName,
    TargetCategory, Timestamp,
};
use sc_observability_types::CorrelationId;
use serde_json::{Map, Value};

use crate::{LogError, LogKind};

/// Largest serialized `fields` object of one row, in bytes.
///
/// A row over the cap is not written: the write returns [`LogError::RowTooLarge`] and the row is
/// counted as lost. 256 KiB is a quarter of sc-observability's 1 MiB event cap
/// (`MAX_LOG_EVENT_BYTES`), leaving ample room for the event envelope, and far above any
/// decision or diagnostic row (callers log ids, sizes and timings, never bodies). With
/// [`QUEUE_CAPACITY`] it also bounds the writer queue's worst-case memory to 128 MiB. Raising it
/// toward 1 MiB makes sc-observability, not this crate, refuse oversized events.
pub const MAX_ROW_BYTES: usize = 256 * 1024;

/// Rows each logger's sc-observability writer queue holds before `write` blocks and `try_write`
/// refuses.
///
/// 512 absorbs a burst of a few hundred rows (a batch of decisions finishing together) without
/// blocking callers on disk, while keeping memory bounded: a typical row is under 4 KiB
/// (about 2 MiB per queue), the worst case is `512 * MAX_ROW_BYTES` = 128 MiB. Lower values make
/// `try_write` refuse earlier under load; higher values hold more unwritten rows at a crash.
pub const QUEUE_CAPACITY: usize = 512;

/// Longest error id written in a last-resort stderr line, in characters (keeps the line bounded).
const MAX_REPORT_ID_CHARS: usize = 128;

/// Whether a write may wait for a free queue slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// `Logger::log`: waits while the writer queue is full.
    Blocking,
    /// `Logger::try_log`: a full queue is an error, counted as a lost row (RSH-010).
    NonBlocking,
}

/// The lost-row accounting of one `Logger`, shared by every writer on it.
#[derive(Debug)]
pub(crate) struct Ledger {
    /// All lost-row state under one lock (RBP-F015).
    accounting: Mutex<Accounting>,
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

impl Ledger {
    /// A ledger for `logger`; losses before now are not reported.
    pub(crate) fn new(logger: &Logger) -> Arc<Self> {
        Arc::new(Self {
            accounting: Mutex::new(Accounting {
                writer_lost_seen: logger.health().dropped_events_total,
                ..Accounting::default()
            }),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Accounting> {
        // Poison: see `Accounting`; the state is a valid count after any panic.
        self.accounting
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Owns or borrows one running logger.
///
/// Dropping an owned writer flushes and shuts the logger down (best effort, with a last-resort
/// stderr line for unreported losses); [`Writer::close`] does the same and returns the outcome.
/// An injected logger is never flushed-on-drop or shut down here (REQ-SAG-0008).
pub(crate) struct Writer {
    logger: Arc<Logger>,
    owned: bool,
    /// Set by `close`, so Drop has nothing left to do.
    closed: bool,
    /// The logger's service; every event carries it (sc-observability refuses another).
    service: ServiceName,
    ledger: Arc<Ledger>,
    kind: LogKind,
}

impl Writer {
    /// A writer that owns `logger`: close or Drop flushes and shuts it down.
    pub(crate) fn owned(logger: Logger, kind: LogKind) -> Self {
        let ledger = Ledger::new(&logger);
        Self::new(Arc::new(logger), true, ledger, kind)
    }

    /// A writer over an injected `logger`, never shut down here. `ledger` is the logger's shared
    /// ledger when other writers use the same logger; `None` starts a new one.
    pub(crate) fn injected(
        logger: Arc<Logger>,
        ledger: Option<Arc<Ledger>>,
        kind: LogKind,
    ) -> Self {
        let ledger = ledger.unwrap_or_else(|| Ledger::new(&logger));
        Self::new(logger, false, ledger, kind)
    }

    fn new(logger: Arc<Logger>, owned: bool, ledger: Arc<Ledger>, kind: LogKind) -> Self {
        Self {
            service: logger.service_name().clone(),
            logger,
            owned,
            closed: false,
            ledger,
            kind,
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
        let bytes = serialized_len(&fields);
        if bytes > MAX_ROW_BYTES {
            return Err(LogError::RowTooLarge {
                log: self.kind,
                bytes,
                max: MAX_ROW_BYTES,
            });
        }
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
            log: self.kind,
            source,
        })
    }

    /// Counts one refused row and keeps it as the latest failure.
    fn count(&self, error: LogError) -> LogError {
        let detail = error.detail();
        let mut accounting = self.ledger.lock();
        accounting.refused = accounting.refused.saturating_add(1);
        accounting.last = Some(detail);
        drop(accounting);
        error
    }

    /// Flush barrier; then [`LogError::RowsLost`] if any row since the last report was
    /// refused or not written. Each lost row is reported once across all writers on the logger.
    pub(crate) fn flush(&self) -> Result<(), LogError> {
        self.logger.flush().map_err(|source| LogError::Flush {
            log: self.kind,
            source,
        })?;
        // Read health before the lock: no call leaves the crate while it is held.
        let health = self.logger.health();
        let total = health.dropped_events_total;
        let mut accounting = self.ledger.lock();
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
            log: self.kind,
            count,
            last: writer_last.or(refused_last),
        })
    }

    /// Flushes, shuts an owned logger down and returns the flush outcome.
    ///
    /// After a successful flush barrier every admitted row is written, so a shutdown failure
    /// loses no row; it is written as a last-resort stderr line, not returned.
    pub(crate) fn close(mut self) -> Result<(), LogError> {
        let flushed = self.flush();
        if self.owned
            && let Err(error) = self.logger.shutdown()
        {
            last_resort(self.kind, None, Some(&error));
        }
        self.closed = true;
        flushed
    }
}

impl fmt::Debug for Writer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Writer")
            .field("kind", &self.kind)
            .field("service", &self.service)
            .field("owned", &self.owned)
            .finish_non_exhaustive()
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        if !self.owned || self.closed {
            return;
        }
        // Best effort: no caller is left to return to, so anything unreported goes to stderr.
        let flushed = self.flush();
        let shutdown = self.logger.shutdown();
        last_resort(self.kind, flushed.err().as_ref(), shutdown.err().as_ref());
    }
}

/// Writes one bounded line to stderr when `error` or `shutdown` is set; nothing otherwise.
///
/// Used where a failure has no caller left: an owned writer dropped without `close`, a shutdown
/// failure after a successful flush, the second error of `Loggers::close`. The line carries the
/// logger, the lost-row count and the error id only (never a message or field value). A failed
/// stderr write is ignored: there is nowhere further to report it.
pub(crate) fn last_resort(
    kind: LogKind,
    error: Option<&LogError>,
    shutdown: Option<&ShutdownError>,
) {
    let mut line = match error {
        None => String::new(),
        Some(LogError::RowsLost { count, last, .. }) => {
            let id = last.as_ref().map_or("unknown", |last| last.id.as_str());
            format!("{count} lost row(s) (last: {})", bounded(id))
        }
        Some(other) => format!("unreported error ({})", bounded(other.id())),
    };
    if let Some(shutdown) = shutdown {
        if !line.is_empty() {
            line.push_str("; ");
        }
        line.push_str("shutdown failed (");
        line.push_str(&bounded(shutdown.diagnostic().code.as_str()));
        line.push(')');
    }
    if line.is_empty() {
        return;
    }
    let _ = writeln!(
        std::io::stderr().lock(),
        "sc-agent-log: {kind} log closed with {line}"
    );
}

fn bounded(id: &str) -> String {
    id.chars().take(MAX_REPORT_ID_CHARS).collect()
}

/// The serialized JSON length of `fields`, counted without allocating the text.
fn serialized_len(fields: &Map<String, Value>) -> usize {
    /// Counts bytes written to it.
    struct Counter(usize);
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.saturating_add(bytes.len());
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter(0);
    // A Map of serde_json Values always serializes; on the impossible error the count so far
    // is used, which is still a lower bound of the size.
    let _ = serde_json::to_writer(&mut counter, fields);
    counter.0
}

/// The logger configuration for `service` at `log_root`: console off, built-in file sink as
/// given, queue capacity [`QUEUE_CAPACITY`].
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
    config.queue_capacity = QUEUE_CAPACITY;
    Ok(config)
}

/// Starts a logger on `config`, with `extra` registered beside the built-in sinks.
pub(crate) fn start_logger(
    config: LoggerConfig,
    log: LogKind,
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
