//! [`SessionLogger`]: one JSONL row per call into the session log (REQ-SAG-0004, REQ-SAG-0005).
//!
//! The default file is `<log_root>/<repo>/<agent_role>/<project>/sessions.jsonl`: one file per
//! directory, appended by every run and every process, kept indefinitely. It is written by a
//! registered `JsonlFileSink` whose policy never renames or deletes it; the sink opens the file
//! in append mode for each row, so concurrent loggers and processes append to the same file.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sc_agent_types::SessionId;
use sc_observability::v2::{JsonlFileSink, Logger, RetainedLogPolicy, SinkRegistration};
use sc_observability::{ByteCount, FileCount, Level, RetentionMaxAge};
use serde_json::{Map, Value};

use crate::writer::{Mode, Writer, logger_config, start_logger};
use crate::{LogError, Repo, Slug};

/// The `log` label of the session logger in errors.
const LOG: &str = "session";

/// Who wrote a session row: repo, project and agent role (REQ-SAG-0004).
///
/// Agent role and agent name are one field, `agent_role` (e.g. `plan-anchor`, `judge-jev`,
/// `sanity`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionIdentity {
    /// The repository the session ran in.
    pub repo: Repo,
    /// The project or phase.
    pub project: Slug,
    /// The type of work, or the agent that did it.
    pub agent_role: Slug,
}

impl SessionIdentity {
    /// Validates `project` and `agent_role` as slugs.
    ///
    /// # Errors
    ///
    /// [`LogError::InvalidSlug`] for either; the text is never rewritten.
    pub fn new(repo: Repo, project: &str, agent_role: &str) -> Result<Self, LogError> {
        Ok(Self {
            repo,
            project: Slug::new("project", project)?,
            agent_role: Slug::new("agent_role", agent_role)?,
        })
    }

    /// `<log_root>/<repo>/<agent_role>/<project>` (REQ-SAG-0005).
    #[must_use]
    pub fn dir(&self, log_root: &Path) -> PathBuf {
        let mut dir = log_root.to_path_buf();
        dir.extend(self.repo.as_str().split('/'));
        dir.push(self.agent_role.as_str());
        dir.push(self.project.as_str());
        dir
    }
}

/// Writes session rows. Clones share one writer.
///
/// # Examples
///
/// ```
/// use sc_agent_log::{Repo, SessionIdentity, SessionLogger};
/// use sc_agent_types::SessionId;
///
/// let root = tempfile::tempdir()?;
/// let repo = Repo::from_origin_url("https://github.com/randlee/sc-agent.git")?;
/// let identity = SessionIdentity::new(repo, "phase-b", "rust-developer")?;
/// let log = SessionLogger::open_default(root.path(), identity)?;
/// let mut payload = serde_json::Map::new();
/// payload.insert("decision".into(), "yes".into());
/// log.write(SessionId::new(), payload)?;
/// log.flush()?;
/// let file = root
///     .path()
///     .join("github.com/randlee/sc-agent/rust-developer/phase-b/sessions.jsonl");
/// assert_eq!(log.path(), Some(file.as_path()));
/// assert!(file.exists());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone)]
pub struct SessionLogger {
    writer: Arc<Writer>,
    identity: Arc<SessionIdentity>,
    path: Option<PathBuf>,
}

impl SessionLogger {
    /// The service name of the default session logger.
    pub const SERVICE: &'static str = "sc-agent-session";
    /// The target of every session row.
    pub const TARGET: &'static str = "sc_agent.session";
    /// The action of every session row.
    pub const ACTION: &'static str = "session.row";
    /// Written first in every row; a payload may not use them.
    pub const IDENTITY_KEYS: [&'static str; 4] = ["session_id", "repo", "project", "agent_role"];
    /// The session log file name in each identity directory.
    pub const FILE: &'static str = "sessions.jsonl";

    /// The default session log: `identity.dir(log_root)/sessions.jsonl`, appended, kept forever.
    ///
    /// sc-observability's built-in file sink and console are off; rows go through a registered
    /// `JsonlFileSink` opened with [`never_rotate`]. The directory is created by the first row.
    ///
    /// # Errors
    ///
    /// [`LogError::OpenSink`] when the file cannot be inspected, [`LogError::Start`] when the
    /// logger cannot start.
    pub fn open_default(log_root: &Path, identity: SessionIdentity) -> Result<Self, LogError> {
        let dir = identity.dir(log_root);
        let file = dir.join(Self::FILE);
        let sink = JsonlFileSink::open(file.clone(), never_rotate()).map_err(|source| {
            LogError::OpenSink {
                path: file.clone(),
                source,
            }
        })?;
        let mut config = logger_config(Self::SERVICE, &dir, false)?;
        config.retained_log_policy = never_rotate();
        let registration = SinkRegistration::typed(Arc::new(sink));
        let logger = start_logger(config, LOG, Some(registration))?;
        Ok(Self {
            writer: Arc::new(Writer::owned(logger, LOG)),
            identity: Arc::new(identity),
            path: Some(file),
        })
    }

    /// A session logger writing to the injected `logger`; it is never flushed-on-drop or shut
    /// down here.
    #[must_use]
    pub fn injected(logger: Arc<Logger>, identity: SessionIdentity) -> Self {
        Self {
            writer: Arc::new(Writer::injected(logger, LOG)),
            identity: Arc::new(identity),
            path: None,
        }
    }

    /// The identity written into every row.
    #[must_use]
    pub fn identity(&self) -> &SessionIdentity {
        &self.identity
    }

    /// The default session file; `None` when injected.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Writes one row: identity fields, then `payload` in its order.
    ///
    /// The row's correlation id is `session_id`. Blocks while the sc-observability writer queue
    /// is full, until the writer thread drains a slot (bounded by disk throughput, not a timer).
    ///
    /// # Errors
    ///
    /// [`LogError::ReservedField`] when `payload` uses a [`Self::IDENTITY_KEYS`] key,
    /// [`LogError::Rejected`] when sc-observability refuses the row. Every Err is also counted
    /// and reported once by [`Self::flush`], so a caller that must not act on it loses nothing.
    pub fn write(
        &self,
        session_id: SessionId,
        payload: Map<String, Value>,
    ) -> Result<(), LogError> {
        self.write_in(Mode::Blocking, session_id, payload)
    }

    /// Same row as [`Self::write`], but never blocks (`Logger::try_log`).
    ///
    /// For the cancel and result paths, where a stalled disk must not delay a result (RSH-010).
    ///
    /// # Errors
    ///
    /// As [`Self::write`]; a full queue is a [`LogError::Rejected`], counted as a lost row.
    pub fn try_write(
        &self,
        session_id: SessionId,
        payload: Map<String, Value>,
    ) -> Result<(), LogError> {
        self.write_in(Mode::NonBlocking, session_id, payload)
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

    fn write_in(
        &self,
        mode: Mode,
        session_id: SessionId,
        payload: Map<String, Value>,
    ) -> Result<(), LogError> {
        let row = self.row(session_id, payload);
        self.writer.write(
            mode,
            Level::Info,
            Self::TARGET,
            Self::ACTION,
            Some(session_id),
            row,
        )
    }

    /// Identity fields first, then `payload` in its order.
    fn row(
        &self,
        session_id: SessionId,
        payload: Map<String, Value>,
    ) -> Result<Map<String, Value>, LogError> {
        if let Some(key) = payload
            .keys()
            .find(|key| Self::IDENTITY_KEYS.contains(&key.as_str()))
        {
            return Err(LogError::ReservedField { key: key.clone() });
        }
        let identity = &self.identity;
        let mut row = Map::with_capacity(payload.len() + Self::IDENTITY_KEYS.len());
        row.insert(
            "session_id".to_owned(),
            Value::String(session_id.to_string()),
        );
        row.insert("repo".to_owned(), Value::from(identity.repo.as_str()));
        row.insert("project".to_owned(), Value::from(identity.project.as_str()));
        row.insert(
            "agent_role".to_owned(),
            Value::from(identity.agent_role.as_str()),
        );
        row.extend(payload);
        Ok(row)
    }
}

impl fmt::Debug for SessionLogger {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionLogger")
            .field("writer", &self.writer)
            .field("identity", &self.identity)
            .field("path", &self.path)
            .finish()
    }
}

/// The session file's policy: it is never renamed or deleted (REQ-SAG-0005).
///
/// rotation_max_bytes = u64::MAX, rotation_max_files = usize::MAX, retention_max_age = 36_500
/// days (100 years: `RetentionMaxAge` must be non-zero, and no file in the directory reaches it).
/// The other fields keep sc-observability's defaults.
fn never_rotate() -> RetainedLogPolicy {
    RetainedLogPolicy {
        rotation_max_bytes: ByteCount::from_bytes(u64::MAX),
        rotation_max_files: FileCount::from_usize(usize::MAX),
        retention_max_age: RetentionMaxAge::from_days(36_500),
        ..RetainedLogPolicy::default()
    }
}
