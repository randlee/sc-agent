//! General and session loggers for sc-agent, over sc-observability 1.5.0 (ADR-SAG-0002).
//!
//! Two separate loggers, both sc-observability v2 [`Logger`]s:
//!
//! - [`GeneralLogger`], the general (diagnostic) log: by default sc-observability's built-in
//!   file `<log_root>/logs/<service>.log.jsonl`, one file per CLI, under sc-observability's own
//!   retention policy (REQ-SAG-0003).
//! - [`SessionLogger`], the session log: one JSONL row per call, appended to one
//!   `<log_root>/logs/<repo>/<agent_role>/<project>/sessions.jsonl` per directory by every run and
//!   process and kept indefinitely (REQ-SAG-0005). Every row's fields begin with `session_id`,
//!   `repo`, `project` and `agent_role` (REQ-SAG-0004).
//!
//! Both take generic rows (`serde_json::Map`); callers build the payload. Either can be an
//! injected `Arc<Logger>`; what is not injected gets the default ([`Loggers`], REQ-SAG-0008).
//! `<log_root>` is `AgentConfig::log_root()` (default `~/.sc-agent`), not decided here.
//!
//! A row a caller cannot act on is still counted: `flush()` reports every refused or lost row
//! once as [`LogError::RowsLost`], and `close()` flushes, shuts the logger down and returns the
//! same report. A default logger dropped without `close()` does that best-effort and writes one
//! bounded last-resort line to stderr (`sc-agent-log: <kind> log closed with ...`) if rows were
//! lost. Rows are capped at [`MAX_ROW_BYTES`]; each logger's writer queue holds
//! [`QUEUE_CAPACITY`] rows. This crate starts no runtime and never sleeps (REQ-SAG-0009,
//! REQ-SAG-0010); its only sc-agent dependency is `sc-agent-types` (ADR-SAG-0004).
//!
//! # Blocking and async callers
//!
//! `write` blocks while the writer queue is full, until the writer thread drains a slot (bounded
//! by disk throughput, not a timer). Code on an async runtime thread must not wait there: it
//! uses the non-blocking paths, [`GeneralLogger::try_write`] and [`SessionLogger::try_write`]
//! (a full queue is an Err counted as a lost row), or runs `write`, `flush` and `close` on a
//! blocking thread (for example `tokio::task::spawn_blocking`). `flush` and `close` wait for the
//! writer thread too. The cancel and result paths of the runner use `try_write` (RSH-010).
//!
//! # Examples
//!
//! ```
//! use sc_agent_log::{Loggers, Repo, SessionIdentity};
//! use sc_agent_types::SessionId;
//!
//! let root = tempfile::tempdir()?;
//! let repo = Repo::local("m5", "work")?;
//! let identity = SessionIdentity::new(repo, "phase-b", "sanity")?;
//! let loggers = Loggers::open(root.path(), "decide", identity, None, None)?;
//! loggers.session.write(SessionId::new(), serde_json::Map::new())?;
//! loggers.session.flush()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
#![forbid(unsafe_code)]

mod context;
mod error;
mod general;
mod kind;
mod loggers;
mod name;
mod repo;
mod session;
mod slug;
mod writer;

#[doc(inline)]
pub use context::LogContext;
#[doc(inline)]
pub use error::LogError;
#[doc(inline)]
pub use general::GeneralLogger;
#[doc(inline)]
pub use kind::LogKind;
#[doc(inline)]
pub use loggers::Loggers;
#[doc(inline)]
pub use name::MAX_SEGMENT_BYTES;
#[doc(inline)]
pub use repo::Repo;
pub use sc_observability::Level;
pub use sc_observability::v2::Logger;
#[doc(inline)]
pub use session::{SessionIdentity, SessionLogger};
#[doc(inline)]
pub use slug::Slug;
#[doc(inline)]
pub use writer::{MAX_ROW_BYTES, QUEUE_CAPACITY};

#[cfg(feature = "testing")]
pub mod testing;
