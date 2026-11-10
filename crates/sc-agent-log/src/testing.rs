//! [`MemoryLog`]: an in-memory log for tests (feature `testing`, not default; PLAN-CRIT-010).
//!
//! sc-observability 1.5.0 has no public in-memory sink (its `RecordingEventSink` is
//! test-private; the public sinks are `JsonlFileSink` and `ConsoleSink`), so `MemoryLog`
//! implements `sc_observability::v2::LogSink` itself and registers it with
//! `SinkRegistration::typed`, with the built-in file and console sinks off. Nothing is written
//! to disk.
//!
//! Consumers take `sc-agent-log = { path = .., features = ["testing"] }` as a dev-dependency.
//!
//! # Examples
//!
//! ```
//! use sc_agent_log::testing::MemoryLog;
//! use sc_agent_log::{Level, LogContext};
//!
//! let memory = MemoryLog::new()?;
//! let log = memory.general();
//! log.write(Level::Warn, "sc_agent_http", "http.busy", LogContext::NONE, serde_json::Map::new())?;
//! let records = memory.records()?;
//! assert_eq!(records.len(), 1);
//! assert_eq!(records[0].action, "http.busy");
//! # Ok::<(), sc_agent_log::LogError>(())
//! ```

use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use sc_observability::v2::{
    LogEvent, LogSink, LogSinkError, Logger, LoggerConfig, SinkRegistration,
};
use sc_observability::{Level, ServiceName, SinkHealth, SinkHealthState, SinkName};
use serde_json::{Map, Value};

use crate::writer::start_logger;
use crate::{GeneralLogger, LogError, SessionIdentity, SessionLogger};

/// The service name of the `MemoryLog` logger.
const SERVICE: &str = "sc-agent-test";
/// The `log` label of `MemoryLog` errors.
const LOG: &str = "memory";

/// In-memory log for tests: loggers built from it write events here, never to a file.
#[derive(Clone)]
pub struct MemoryLog {
    sink: Arc<MemorySink>,
    logger: Arc<Logger>,
}

/// The sink: every written event, in write order.
struct MemorySink {
    /// Only pushed to or cloned under the lock, so a poisoned lock still holds a valid Vec.
    events: Mutex<Vec<LogEvent>>,
    /// `"memory"`, reported by `health()`.
    name: SinkName,
}

impl MemorySink {
    fn events(&self) -> MutexGuard<'_, Vec<LogEvent>> {
        self.events.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl LogSink for MemorySink {
    fn write(&self, event: &LogEvent) -> Result<(), LogSinkError> {
        self.events().push(event.clone());
        Ok(())
    }

    fn health(&self) -> SinkHealth {
        SinkHealth {
            name: self.name.clone(),
            state: SinkHealthState::Healthy,
            last_error: None,
        }
    }
}

impl MemoryLog {
    /// Builds one logger with a `MemoryLog` sink, file and console sinks off.
    ///
    /// # Errors
    ///
    /// [`LogError::Start`] when the logger cannot start, [`LogError::InvalidName`] for the
    /// service or sink name (never in practice: both are constant valid names).
    pub fn new() -> Result<Self, LogError> {
        let service = ServiceName::new(SERVICE)
            .map_err(|error| LogError::invalid_name("service", SERVICE, &error))?;
        let mut config = LoggerConfig::default_for(service, PathBuf::new());
        config.enable_file_sink = false;
        config.enable_console_sink = false;
        let name =
            SinkName::new(LOG).map_err(|error| LogError::invalid_name("sink", LOG, &error))?;
        let sink = Arc::new(MemorySink {
            events: Mutex::new(Vec::new()),
            name,
        });
        let registration = SinkRegistration::typed(Arc::clone(&sink) as Arc<dyn LogSink>);
        let logger = start_logger(config, LOG, Some(registration))?;
        Ok(Self {
            sink,
            logger: Arc::new(logger),
        })
    }

    /// A [`GeneralLogger`] writing into this log ([`GeneralLogger::injected`]).
    #[must_use]
    pub fn general(&self) -> GeneralLogger {
        GeneralLogger::injected(Arc::clone(&self.logger))
    }

    /// A [`SessionLogger`] writing into this log ([`SessionLogger::injected`]).
    #[must_use]
    pub fn session(&self, identity: SessionIdentity) -> SessionLogger {
        SessionLogger::injected(Arc::clone(&self.logger), identity)
    }

    /// The logger itself, for [`Loggers::open`](crate::Loggers::open) injection.
    #[must_use]
    pub fn logger(&self) -> Arc<Logger> {
        Arc::clone(&self.logger)
    }

    /// Flushes the logger, then returns every event written so far, in write order.
    ///
    /// # Errors
    ///
    /// [`LogError::Flush`] when the flush barrier fails.
    pub fn records(&self) -> Result<Vec<MemoryRecord>, LogError> {
        self.logger
            .flush()
            .map_err(|source| LogError::Flush { log: LOG, source })?;
        let events = self.sink.events().clone();
        Ok(events.into_iter().map(MemoryRecord::from).collect())
    }

    /// Every written event whole (correlation id included), in write order, after a flush.
    ///
    /// # Errors
    ///
    /// [`LogError::Flush`] when the flush barrier fails.
    pub fn events(&self) -> Result<Vec<LogEvent>, LogError> {
        self.logger
            .flush()
            .map_err(|source| LogError::Flush { log: LOG, source })?;
        Ok(self.sink.events().clone())
    }
}

impl fmt::Debug for MemoryLog {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MemoryLog")
            .field("events", &self.sink.events().len())
            .finish_non_exhaustive()
    }
}

/// One event of a [`MemoryLog`].
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryRecord {
    /// The event level.
    pub level: Level,
    /// The event target, e.g. `sc_agent_http`.
    pub target: String,
    /// The event action, e.g. `http.busy`.
    pub action: String,
    /// The event fields, in write order.
    pub fields: Map<String, Value>,
}

impl From<LogEvent> for MemoryRecord {
    fn from(event: LogEvent) -> Self {
        Self {
            level: event.level,
            target: event.target.as_str().to_owned(),
            action: event.action.as_str().to_owned(),
            fields: event.fields,
        }
    }
}
