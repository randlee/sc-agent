<!-- Generated from bead agt-4p2.8 for file-based reviewers. The bead is the source of truth: edit the bead, then re-export. -->

# Sprint A-8: sc-agent-log
- id: `a-8` (bead `agt-4p2.8`)
- branch: `sprint/a-8-sc-agent-log`
- depends on: a-1
- copy script: `docs/plans/phase-a/copy/a-8.sh`

## Description

Separate general and session loggers on sc-observability, injectable with defaults; general log one file per CLI under <log_root>/logs/; session JSONL appended to one sessions.jsonl per <log_root>/<repo>/<agent_role>/<project>/, kept indefinitely; <log_root> defaults to ~/.sc-agent. Records: REQ-SAG-0003, 0004, 0005, 0008, NFR-SAG-0001, ADR-SAG-0002.

## Design

```yaml
sprint:
  id: a-8
  closure_type: boundary
  target_boundary: sc-agent-log
  owned_paths: [crates/sc-agent-log/**]
  must_follow:
    - sprint: a-1
      artifact: sc_agent_types::{SessionId, ErrorId, ErrorDetail} + workspace root Cargo.toml and clippy.toml
      trigger: dev-push
  requirements: [REQ-SAG-0003, REQ-SAG-0004, REQ-SAG-0005, REQ-SAG-0008, NFR-SAG-0001, REQ-SAG-0001, REQ-SAG-0002, REQ-SAG-0009, REQ-SAG-0010]
  adrs: [ADR-SAG-0002, ADR-SAG-0004]
  does_not_close:
    - "REQ-SAG-0004 slug rejection end to end (decide and sc-agent-py refuse project \"Phase B\" with log.invalid_slug and write nothing): a-10 acceptance"
    - "REQ-SAG-0004 AC3 end to end (the CLI and PyO3 inputs are only project and agent_role): a-10 acceptance"
    - "REQ-SAG-0003 per-CLI file in use (decide opens GeneralLogger::open_default(.., \"decide\"), tsq its own): a-10 and a-3; the ~/.sc-agent default of <log_root> is AgentConfig::log_root, a-4"
    - "REQ-SAG-0008 end to end (logger injection through decide's options and sc-agent-py): a-10 acceptance"
```

# Sprint A-8: sc-agent-log (bead agt-4p2.8)

Governing: REQ-SAG-0003, REQ-SAG-0004, REQ-SAG-0005, REQ-SAG-0008, NFR-SAG-0001, ADR-SAG-0002. Also binding: REQ-SAG-0001, REQ-SAG-0002, REQ-SAG-0009, REQ-SAG-0010, ADR-SAG-0004 (layer: directly above sc-agent-types, beside sc-agent-config/sc-agent-secrets, below sc-agent-http; takes generic rows, knows no decision types).
Owner decisions: owner-logger-injection, owner-logging-retention, owner-session-log-layout, owner-ids-ulid, owner-error-contract, owner-no-sleep, owner-lowlevel-no-tokio, owner-architecture.

Copy script: `docs/plans/phase-a/copy/a-8.sh` (dry run 2026-11-09, reconciliation: 8 copy steps, 7 files, 431 lines). Target: `crates/sc-agent-log/`.

## Shape in one paragraph

Two separate loggers, both `sc-observability` 1.5.0 v2 `Logger`s. `GeneralLogger` is the general log: default is sc-observability's built-in file sink at `<log_root>/logs/<service>.log.jsonl`, one file per CLI (`decide`; `sc-agent-py` for the Python API), with its default `RetainedLogPolicy` and level (rotation and retention are sc-observability's, REQ-SAG-0003). `<log_root>` is `AgentConfig::log_root()` (A-4), default `~/.sc-agent`, sc-observability's standard root for app sc-agent. `SessionLogger` writes one JSONL row per call into `<log_root>/<repo>/<agent_role>/<project>/sessions.jsonl`: one file per directory, appended by every run and process and never rotated (REQ-SAG-0005), through a registered `JsonlFileSink` whose policy never rotates or prunes (concurrent appends are sc-observability's); its built-in file sink is off. Every session row's `fields{}` begins with `session_id`, `repo`, `project`, `agent_role` (agent role and agent name are one field, REQ-SAG-0004). Both loggers take generic rows (`serde_json::Map`); callers build the payload (the runner builds the decision row, A-9; sc-agent-http writes its events through an injected `GeneralLogger`, A-5). Either logger can be an injected `Arc<sc_observability::v2::Logger>`; what is not injected gets the default. A write a caller cannot act on (a log failure must never change a send or decision outcome) is still counted: `flush()` reports every refused or lost row once.

## 1. Copy list (source `sc-agent-ref@dce239a` -> `crates/sc-agent-log/`)

| # | Source path:lines | Target | What | Required edits after copying |
|---|---|---|---|---|
| C1 | `crates/sc-jev-host/LICENSE` | `LICENSE` | MIT | none |
| C2 | `crates/sc-jev-host/src/obs.rs:107-209` | `src/writer.rs` | `LoggerInner`: owns `Arc<Logger>`, `flush`, `take_lost_rows`, Drop = flush + shutdown | Rename `Writer`. Delete `pid`, `is_local`, the fork `blocking` helper, the leak branch of Drop, `logs_dir`/`file_name`/`harden_log_files`. Add `owned: bool` (Drop flushes and shuts down only an owned logger) and one `accounting: Mutex<Accounting>` holding all lost-row state (RBP-F015). `flush` -> `LogError::Flush`, then `LogError::RowsLost` when refused + newly lost rows > 0, read and reset under that one lock (reported once). `JevError` gone. |
| C3 | `crates/sc-jev-host/src/obs.rs:418-455` | `src/writer.rs` | `logger_config`, `start_logger` | `logger_config(service, log_root, file_sink: bool)`; delete the serde `LevelFilter` hack. `start_logger(config, extra: Option<SinkRegistration>)` -> `LogError::Start`. Delete the hint text and the `prepare_log_files` call. |
| C4 | `crates/sc-jev-host/src/obs.rs:486-528` | `src/general.rs` | `prepare_log_files`: one startup `JsonlFileSink::open(active, policy)` so a short CLI still rotates | Keep only that call; error -> `LogError::OpenSink`. Delete `create_private_dirs`, 0600 creation, `harden_log_files`. Not rotation code (REQ-SAG-0003). |
| C5 | `crates/sc-jev-host/src/obs.rs:310-349` | `src/writer.rs` | `log_event`: builds the v2 `LogEvent` | Private `Writer::event(level, target, action, fields) -> Result<LogEvent, LogError>`, shared by both loggers. `message: None`, `request_id: None`, `correlation_id` = the LogContext's session_id (`CorrelationId::new(&session_id.to_string())`), else None (delete `envelope_id!` and its `debug_assert`). Name errors -> `LogError::InvalidName`. Delete `impl SessionLog`, `level_of`, `action_name`, `Operation`, the registry, every `GeneralLogAdapter` item. |
| C6 | `crates/sc-agent-types/src/ids.rs:1164-1245` | `src/slug.rs` | `ProjectLabel(Arc<str>)` | Rename `Slug`; rule = REQ-SAG-0004 (non-empty, only `a-z 0-9 . _ -`); error `InvalidSlug { field, value }`; delete `MAX_BYTES`, `FromStr`, serde, doc example; `new(field: &'static str, text: &str)`. |
| C7 | `crates/sc-agent-types/src/git.rs:88-109` | `src/repo.rs` | `git(dir)`: argv-only `git --no-pager -C <dir>`, `GIT_TERMINAL_PROMPT=0`, `LC_ALL=C`, repo-redirect vars removed, stdin null | `#[expect]` reason "the one git spawn of sc-agent-log"; run with `Command::output()` (the old `wait_bounded` slept, REQ-SAG-0010). |
| T1 | `crates/sc-jev-host/tests/obs.rs:202-283` | `tests/writer.rs` | `make_unwritable`; lost rows reported once; healthy writer loses nothing; oversized general log rotates at start | Port to the new loggers; `flush()` returning `Err(RowsLost{count: 2, ..})` once replaces `take_lost_rows()`; the rotation test targets `GeneralLogger::open_default`; write the missing helpers. |

Not copied: the `SessionLog`/`GeneralLog` ports and `NoopSessionLog` (REQ-SAG-0008: callers use the loggers here); the writer registry and fork handling (`obs.rs:55-105, 211-217, 368-416`); `GeneralLogAdapter` (`obs.rs:574-743`); 0600/0700 file modes; `sc-decision-schema/src/session_log/*` (old row schema; ADR-SAG-0001 rejects it; SQLite is later, ADR-SAG-0002); `paths.rs` `PathPolicy` (`<log_root>` is `AgentConfig.log_root`, A-4); `git.rs` deadline/poll code (sleeps).

## 2. Exact public Rust surface (bodies elided)

```rust
// src/lib.rs
#![forbid(unsafe_code)]
mod context; mod error; mod general; mod loggers; mod repo; mod session; mod slug; mod writer;
pub use context::LogContext;
pub use error::LogError;
pub use general::GeneralLogger;
pub use loggers::Loggers;
pub use repo::Repo;
pub use session::{SessionIdentity, SessionLogger};
pub use slug::Slug;
pub use sc_observability::v2::{Level, Logger};
#[cfg(feature = "testing")]
pub mod testing;   // MemoryLog (in-memory GeneralLogger/SessionLogger for consumers' ANC-007 tests)

// src/slug.rs
/// Lowercase text of only `a-z`, `0-9`, `.`, `_` and `-`, non-empty (REQ-SAG-0004). Never rewritten.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Slug(Arc<str>);
impl Slug {
    pub fn new(field: &'static str, text: &str) -> Result<Self, LogError>;   // field: "project" | "agent_role"
    pub fn as_str(&self) -> &str;
}
impl fmt::Display for Slug {}
impl AsRef<str> for Slug {}                                  // RBP-F017; no Deref

// src/repo.rs
/// `<host>/<owner>/<repo>` from the git origin URL, else `local/<host>/<folder-name>` (REQ-SAG-0004).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Repo(Arc<str>);
impl Repo {
    /// `git config --get remote.origin.url` in `dir`; origin -> `from_origin_url`; not a work tree, no origin, or a
    /// local-path / file:// origin -> `local(host, work-tree or dir folder name)`; git not installed -> GitUnavailable.
    pub fn detect(dir: &Path) -> Result<Self, LogError>;
    /// `git rev-parse --show-toplevel` in `dir`; None outside git. The repo root whose `.env` A-4 reads.
    pub fn work_tree_root(dir: &Path) -> Result<Option<PathBuf>, LogError>;
    /// Pure. `scheme://[user[:pass]@]host[:port]/path` or scp `[user@]host:path`; drops scheme, userinfo, port;
    /// strips one trailing `/` then one `.git`; every segment non-empty, not `.`/`..`, no `\` or control chars.
    pub fn from_origin_url(url: &str) -> Result<Self, LogError>;
    pub fn local(host: &str, folder: &str) -> Result<Self, LogError>;
    pub fn as_str(&self) -> &str;
}
impl fmt::Display for Repo {}
impl AsRef<str> for Repo {}                                  // RBP-F017; no Deref

// src/session.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionIdentity { pub repo: Repo, pub project: Slug, pub agent_role: Slug }   // agent_role e.g. plan-anchor, judge-jev, sanity
impl SessionIdentity {
    /// Validates project and agent_role as slugs; InvalidSlug, never a rewrite.
    pub fn new(repo: Repo, project: &str, agent_role: &str) -> Result<Self, LogError>;
    /// `<log_root>/<repo>/<agent_role>/<project>` (REQ-SAG-0005).
    pub fn dir(&self, log_root: &Path) -> PathBuf;
}

/// Writes session rows. Clones share one writer.
#[derive(Clone)]
pub struct SessionLogger { writer: Arc<Writer>, identity: Arc<SessionIdentity>, path: Option<PathBuf> }
impl SessionLogger {
    pub const SERVICE: &'static str = "sc-agent-session";
    pub const TARGET: &'static str = "sc_agent.session";
    pub const ACTION: &'static str = "session.row";
    /// Written first in every row; a payload may not use them.
    pub const IDENTITY_KEYS: [&'static str; 4] = ["session_id", "repo", "project", "agent_role"];
    pub const FILE: &'static str = "sessions.jsonl";
    /// Default: `identity.dir(log_root)/sessions.jsonl` (one file per directory, appended, REQ-SAG-0005), JsonlFileSink with never_rotate().
    pub fn open_default(log_root: &Path, identity: SessionIdentity) -> Result<Self, LogError>;
    /// Injected: rows go to `logger`; never flushed-on-drop or shut down here.
    pub fn injected(logger: Arc<Logger>, identity: SessionIdentity) -> Self;
    pub fn identity(&self) -> &SessionIdentity;
    pub fn path(&self) -> Option<&Path>;                   // None when injected
    /// One row: identity fields, then `payload` in its order; the event correlation id is `session_id`.
    /// Blocks while the sc-observability writer queue is full (until the writer thread drains a slot; bounded by
    /// disk throughput, not by a timer). An Err is also counted, so a caller that must not act on it loses nothing:
    /// flush() reports it.
    pub fn write(&self, session_id: SessionId, payload: serde_json::Map<String, serde_json::Value>) -> Result<(), LogError>;
    /// Same row, never blocks (`Logger::try_log`): a full queue is an Err counted as a lost row. For the cancel
    /// and result paths (a-9) where a stalled disk must not delay a result (RSH-010).
    pub fn try_write(&self, session_id: SessionId, payload: serde_json::Map<String, serde_json::Value>) -> Result<(), LogError>;
    /// Flush barrier; then RowsLost if any row since the last flush was refused or not written.
    pub fn flush(&self) -> Result<(), LogError>;
}
/// rotation_max_bytes = u64::MAX, rotation_max_files = usize::MAX, retention_max_age = 36_500 days.
fn never_rotate() -> RetainedLogPolicy;

// src/general.rs
/// The general (diagnostic) log. Clones share one writer.
#[derive(Clone)]
pub struct GeneralLogger { writer: Arc<Writer> }
impl GeneralLogger {
    /// Default: built-in file sink `<log_root>/logs/<service>.log.jsonl` (`service` = the CLI name, e.g. "decide";
    /// one file per CLI, REQ-SAG-0003), RetainedLogPolicy::default(), default level, console off, one startup retention pass (C4).
    pub fn open_default(log_root: &Path, service: &'static str) -> Result<Self, LogError>;
    pub fn injected(logger: Arc<Logger>) -> Self;
    /// Written first in an event that has a context; a payload may not use them (LogError::ReservedField).
    pub const CONTEXT_KEYS: [&'static str; 2] = ["session_id", "position"];
    /// One event. `target` names the calling crate ("sc_agent_http", "sc_decision_runner"); `action`
    /// e.g. "http.busy". `ctx` (RSH-009): its session_id becomes `LogEvent.correlation_id` and the first field
    /// `session_id`, its position the field `position`; `LogContext::NONE` adds neither. Never a body, header
    /// value or key (callers' rule). Blocks like SessionLogger::write; Errs are counted the same way.
    pub fn write(&self, level: Level, target: &'static str, action: &'static str, ctx: LogContext, fields: serde_json::Map<String, serde_json::Value>) -> Result<(), LogError>;
    /// Same event, never blocks (Logger::try_log); a full queue is counted as a lost row (RSH-010).
    pub fn try_write(&self, level: Level, target: &'static str, action: &'static str, ctx: LogContext, fields: serde_json::Map<String, serde_json::Value>) -> Result<(), LogError>;
    pub fn flush(&self) -> Result<(), LogError>;
}

// src/context.rs
/// Which request a general event belongs to (RSH-009). Passed by value to GeneralLogger::write and to
/// sc_agent_http::HttpClient::send_once; local.*, llm.* and runner.* events use the same type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LogContext {
    pub session_id: Option<SessionId>,
    /// 0-based position of the request in its batch or stream; None for a single request.
    pub position: Option<u64>,
}
impl LogContext {
    pub const NONE: Self = Self { session_id: None, position: None };
    pub const fn request(session_id: SessionId, position: Option<u64>) -> Self;
}

// src/loggers.rs
/// REQ-SAG-0008: inject either, both or neither; anything not injected is the default.
pub struct Loggers { pub general: GeneralLogger, pub session: SessionLogger }
impl Loggers {
    pub fn open(log_root: &Path, service: &'static str, identity: SessionIdentity, general: Option<Arc<Logger>>, session: Option<Arc<Logger>>) -> Result<Self, LogError>;
}

// src/writer.rs (crate-private)
pub(crate) struct Writer { logger: Arc<Logger>, owned: bool, accounting: Mutex<Accounting>, log: &'static str }
/// All lost-row state under one lock (RBP-F015): flush reads and resets it in one critical section.
struct Accounting {
    refused: u64,                 // rows write/try_write returned Err for since the last report
    writer_lost_seen: u64,        // sc-observability's lost-row total already reported
    last: Option<ErrorDetail>,    // latest failure, taken by the report
}
```

Lock and poison rationale (RBP-F015): `Writer.accounting` is mutated only by a counter add or one assignment per critical section, and no call leaves the crate while it is held (the `Logger` call happens before the lock is taken), so after a panic elsewhere the state is still a valid count and `PoisonError::into_inner` is safe. `MemoryLog`'s event Vec is only pushed to or cloned under its lock, so `into_inner` is safe there too.

```rust
```

`SessionId` is `sc_agent_types::SessionId`; `ErrorDetail`/`ErrorId` are A-1's.

```rust
// src/testing.rs (feature "testing", not default; PLAN-CRIT-010)
/// In-memory log for tests. sc-observability 1.5.0 has no public in-memory sink (its RecordingEventSink is
/// test-private; the public sinks are JsonlFileSink and ConsoleSink), so MemoryLog implements
/// `sc_observability::v2::LogSink` itself (write: push a clone of the `LogEvent`; flush: Ok; health:
/// `SinkHealth { name: SinkName "memory", state: SinkHealthState::Healthy, last_error: None }`).
#[derive(Clone)]
pub struct MemoryLog { sink: Arc<MemorySink>, logger: Arc<Logger> }
struct MemorySink { events: Mutex<Vec<LogEvent>> }          // private; impl sc_observability::v2::LogSink
impl MemoryLog {
    /// Builds one Logger: `LoggerConfig::default_for(ServiceName::new("sc-agent-test")?, PathBuf::new())` with
    /// `enable_file_sink = false`, `enable_console_sink = false`;
    /// `v2::LoggerBuilder::new(config)?.register_sink(SinkRegistration::typed(sink.clone())).build()`. Err: LogError::Start / InvalidName.
    pub fn new() -> Result<Self, LogError>;
    /// A GeneralLogger writing into this log (GeneralLogger::injected; never shut down by it).
    pub fn general(&self) -> GeneralLogger;
    /// A SessionLogger writing into this log (SessionLogger::injected).
    pub fn session(&self, identity: SessionIdentity) -> SessionLogger;
    /// Flushes the logger, then returns every event written so far, in write order.
    pub fn records(&self) -> Result<Vec<MemoryRecord>, LogError>;
}
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryRecord { pub level: Level, pub target: String, pub action: String, pub fields: serde_json::Map<String, serde_json::Value> }
impl sc_observability::v2::LogSink for MemorySink {}
```
Consumers (a-5, a-6, a-7, a-9, a-10 ANC-007 tests) take `sc-agent-log = { path = .., features = ["testing"] }` as a dev-dependency. The poisoned-mutex case recovers with `into_inner` (no unwrap; rationale above).

## 3. Error union (full list; ErrorId, Display = `id: message` only (RBP-F013), source() = the wrapped error)

```rust
#[derive(Debug)]
#[non_exhaustive]
pub enum LogError {
    InvalidSlug { field: &'static str, value: String },                       // "log.invalid_slug"
    /// `value` never contains userinfo (dropped before the check).
    InvalidRepo { value: String, reason: &'static str },                      // "log.invalid_repo"
    /// git could not be started.
    GitUnavailable { source: std::io::Error },                                // "log.git_unavailable"
    /// git printed output that is not UTF-8.
    GitOutput { command: &'static str },                                      // "log.git_output"
    /// hostname::get() failed (`source`) or is not UTF-8 (`None`).
    HostUnknown { source: Option<std::io::Error> },                           // "log.host_unknown"
    /// The working folder has no final name component.
    FolderUnknown { path: PathBuf },                                          // "log.folder_unknown"
    /// sc-observability refused a service/target/action name; cause = its id and message.
    InvalidName { what: &'static str, value: String, cause: ErrorDetail },     // "log.invalid_name"
    OpenSink { path: PathBuf, source: sc_observability::v2::InitError },      // "log.open_sink"
    Start { log: &'static str, source: sc_observability::v2::InitError },     // "log.start"
    /// A payload key is one of SessionLogger::IDENTITY_KEYS or GeneralLogger::CONTEXT_KEYS.
    ReservedField { key: String },                                            // "log.reserved_field"
    /// Logger::log refused the row.
    Rejected { log: &'static str, source: sc_observability::v2::EventError }, // "log.rejected"
    Flush { log: &'static str, source: sc_observability::v2::FlushError },    // "log.flush"
    /// `count` rows since the last flush were refused or not written; `last` is the latest failure.
    RowsLost { log: &'static str, count: u64, last: Option<ErrorDetail> },     // "log.rows_lost"
}
impl ErrorId for LogError {}   // cause_detail() (a-1 rename, RBP-F001): foreign sources become ErrorDetail::foreign causes; recovery() None
                               // (no LogError is on the RBP-F005 list)
```

13 variants.

## 4. New code (not in the copy)

- N1 `Cargo.toml` (`[lints] workspace = true`), `README.md`, `src/lib.rs`, `src/error.rs`.
- N2 `src/repo.rs` (~80 lines): `detect`, `work_tree_root`, `from_origin_url`, `local`, segment check, short host.
  Decisions (assistant; owner: IDC): host = `hostname::get()` text before the first `.` (`m5.local` -> `m5`); no origin or a local-path / file:// origin -> `local/<host>/<work-tree folder>`; git not installed -> `log.git_unavailable`; the port in `ssh://host:22/...` is dropped; host case is kept.
- N3 `src/session.rs` (~90 lines): `SessionIdentity`, `SessionLogger`, `never_rotate()`; `LoggerConfig::default_for(ServiceName::new(SERVICE), identity dir)` with file and console sinks off plus `SinkRegistration::typed(Arc::new(JsonlFileSink::open(file, never_rotate())?))`.
- N4 `src/general.rs` (~45 lines), N5 `src/loggers.rs` (~20 lines).
- No config keys: `<log_root>` comes from `AgentConfig::log_root()` (A-4).

## 5. Tests

Each test names the acceptance line (A8-nn) it proves; the assertion itself is in that line.

| Test | Proves |
|---|---|
| `slug_rules` (`phase-b`, `rust-developer`, `a.b_c-1` ok; `Phase B`, `""`, `a/b`, `Rust` refused), `invalid_project_writes_nothing` | A8-08 |
| `origin_forms` (https, scp, `ssh://git@github.com:22/...`, userinfo URL; `https://h/../x` refused), `detect_origin` (`git init` + `git remote add origin`) | A8-05 |
| `local_repo`, `repo_edge_cases` (no origin, local-path origin, `m5.local` -> `m5`) | A8-06 |
| `identity_inputs` (SessionIdentity::new takes only repo, project, agent_role; no agent_name key in any row) | A8-07 |
| `rows_under_layout`, `runs_append_one_file` | A8-09 |
| `rows_self_describe` | A8-10 |
| `forty_thousand_sessions` | A8-11 |
| `general_file_per_cli` | A8-03 |
| `session_file_never_rotates`, `old_session_log_survives` | A8-04 |
| `general_rotates_at_start` (T1 port) | A8-02 |
| `general_write_generic` | A8-13 |
| `context_on_general_events` (two LogContexts -> different correlation_id and session_id fields; NONE adds neither; a payload `session_id` key -> log.reserved_field) | A8-20 |
| `try_write_never_blocks` (a full queue -> Err, counted, reported once by flush) | A8-21 |
| `defaults_write_both`, `injected_session_only`, `injected_general_only`, `injected_logger_not_shut_down` | A8-12 |
| `reserved_field_refused_and_counted`, `rows_lost_reported_once` (unix), `healthy_writer_loses_nothing`, `concurrent_refusals_counted_once` (threads refuse while another flushes; reported total = refusals) | A8-14 |
| `log_error_keeps_lower_layer_cause` | A8-15 |
| `ids` | A8-16 |
| `memory_log_records` (feature `testing`: GeneralLogger and SessionLogger from MemoryLog; records() returns level, target, action, fields in order; no file under any root) | A8-19 |

Not proved by a test here: A8-01 (commit history), A8-17/A8-18 (Required validation commands below).

## Required validation

- `cargo build -p sc-agent-log` and `cargo build -p sc-agent-log --features testing`
- `cargo clippy -p sc-agent-log --all-targets --features testing -- -D warnings` (workspace denies: panic, unwrap_used, expect_used, indexing_slicing; disallowed-methods incl. every sleep)
- `cargo test -p sc-agent-log --features testing`
- `cargo tree -p sc-agent-log -e normal` (and with `--features testing`): no tokio; the only sc-agent crate is sc-agent-types; no http, decision or runner crate
- `grep -rnE "sleep\(" crates/sc-agent-log` finds nothing; `grep -rniE "rotat|prune" crates/sc-agent-log/src` shows only the never_rotate() policy values (A8-02)

## 6. Dependencies

- `sc-agent-types` (path, A-1): `SessionId`, `ErrorId`, `ErrorDetail`.
- `sc-observability = "=1.5.0"` (ref-src pin): v2 `Logger`, `LoggerBuilder`, `LoggerConfig`, `JsonlFileSink`, `SinkRegistration`, `RetainedLogPolicy`, names, `Level`, `LogEvent`, `Logger::log` (blocking) and `Logger::try_log` (non-blocking).
- `sc-observability-types = "=1.5.0"`: `CorrelationId` (not re-exported by sc-observability).
- `serde_json = "1"` (lock 1.0.154), `hostname = "0.4.2"` (new).
- dev: `tempfile = "3"`; self dev-dependency `sc-agent-log = { path = ".", features = ["testing"] }`.
- Feature `testing` (not default): enables `src/testing.rs` (`MemoryLog`); no extra dependencies.
- No tokio, no thiserror, no sc-decision-* crate.

## Acceptance criteria

- [ ] A8-01 Copy first: the sprint's first commit is the unchanged output of `docs/plans/phase-a/copy/a-8.sh` (sc-agent-ref@dce239a), committed before any edit; every change needed to make that code build and pass comes in later commits (owner-copy-mechanically).
- [ ] A8-02 REQ-SAG-0003 AC1: sc-agent-log: GeneralLogger::open_default's general log rotates under sc-observability's RetainedLogPolicy (startup pass test), and no sc-agent-log source contains rotation or pruning code.
- [ ] A8-03 REQ-SAG-0003: sc-agent-log: GeneralLogger::open_default(log_root, service) writes <log_root>/logs/<service>.log.jsonl, one file per CLI name (the ~/.sc-agent default of <log_root> is AgentConfig::log_root, a-4).
- [ ] A8-04 REQ-SAG-0003 AC2: sc-agent-log: a session log file older than the retention age (mtime 8 days back) is still present after a new SessionLogger writes and flushes in the same directory; session files never rotate (an over-64 MiB session file is not renamed).
- [ ] A8-05 REQ-SAG-0004 AC1: sc-agent-log: Repo::detect in a clone whose origin is https://github.com/randlee/sc-agent.git gives repo = github.com/randlee/sc-agent and SessionLogger rows carry it (also scp and ssh forms; userinfo never appears).
- [ ] A8-06 REQ-SAG-0004 AC2: sc-agent-log: outside git, folder work on host m5 gives repo = local/m5/work.
- [ ] A8-07 REQ-SAG-0004 AC3: sc-agent-log: project and agent_role are the only caller inputs of SessionIdentity::new besides the detected Repo (no agent_name anywhere in sc-agent-log); the CLI/PyO3 end-to-end check is a-10's.
- [ ] A8-08 REQ-SAG-0004 AC4: sc-agent-log: SessionIdentity::new with project "Phase B" returns log.invalid_slug and no row or file is written.
- [ ] A8-09 REQ-SAG-0005 AC: sc-agent-log: SessionLogger rows for repo github.com/randlee/sc-agent, agent_role rust-developer, project phase-b are in <log_root>/github.com/randlee/sc-agent/rust-developer/phase-b/sessions.jsonl; every later run and every concurrent logger with that identity appends to the same file (test).
- [ ] A8-10 ADR-SAG-0002 / REQ-SAG-0001: sc-agent-log: every SessionLogger row's fields carry session_id, repo, project and agent_role (one field for role and name); session_id parses as a ULID.
- [ ] A8-11 NFR-SAG-0001: sc-agent-log: 40,000 SessionLogger writes with distinct session_id give 40,000 rows in one file.
- [ ] A8-12 REQ-SAG-0008 AC1-3: sc-agent-log: Loggers::open with no injection -> both default files written; injected session logger -> rows there, general log default; injected general logger -> general log there, rows in the default session file. An injected logger is never shut down by sc-agent-log.
- [ ] A8-13 ADR-SAG-0004: sc-agent-log sits in the config, secrets and log layer; its only sc-agent crate dependency is sc-agent-types, and it depends on no http, decision-schema, decision-backend or runner crate, checked by `cargo tree -p sc-agent-log` (with and without `--features testing`); both loggers take generic rows (serde_json::Map).
- [ ] A8-14 sc-agent-log: rows refused at write or lost by the writer are returned once as log.rows_lost from flush(), with the latest failure as ErrorDetail; never lost silently.
- [ ] A8-15 REQ-SAG-0002: every LogError returned by a public sc-agent-log function carries the id and message of each lower-layer cause (sc-observability error or io error), kept as its source and in ErrorDetail, never dropped or replaced; test log_error_keeps_lower_layer_cause injects an sc-observability failure and an io failure (unwritable <log_root>) and asserts the returned LogError exposes that cause's id and message through source() and ErrorDetail.
- [ ] A8-16 REQ-SAG-0002: LogError implements ErrorId with 13 distinct log.* ids; clippy with the workspace denies (panic, unwrap_used, expect_used, indexing_slicing) passes for sc-agent-log.
- [ ] A8-17 REQ-SAG-0009/0010: cargo tree -p sc-agent-log shows no tokio; no sleep call (git runs via Command::output()).
- [ ] A8-18 REQ-SAG-0010: sc-agent-log source calls no sleep function (std::thread::sleep, tokio::time::sleep or any other), enforced by the workspace clippy disallowed-methods rule.
- [ ] A8-19 sc-agent-log: feature `testing` (not default) exposes sc_agent_log::testing::MemoryLog, a sc_observability::v2::LogSink registered through SinkRegistration::typed with file and console sinks off; GeneralLogger and SessionLogger built from it write no file, and records() (after its flush) returns each event's level, target, action and fields in write order.
- [ ] A8-20 sc-agent-log: GeneralLogger::write takes a LogContext; its session_id is the event's correlation_id and first field, its position the field position; LogContext::NONE adds neither; two contexts give records with different session_ids.
- [ ] A8-21 sc-agent-log: GeneralLogger::try_write and SessionLogger::try_write never block; a row they cannot queue is counted and returned once by flush() as log.rows_lost (all lost-row accounting under one Mutex).

## Notes

Closed 2026-11-09 by owner answers (search-for-truth step 7); decisions are in the design:
- Q1 one sessions.jsonl per <repo>/<agent_role>/<project> directory, appended forever (owner; REQ-SAG-0005); not per run or session.
- Q2/Q3 host and repo edge cases: assistant decisions (owner: IDC).
- Q4 general log: <log_root>/logs/<cli>.log.jsonl per CLI, sc-observability default level and retention; <log_root> default ~/.sc-agent (owner; REQ-SAG-0003).
No open questions.

Plan hardening round 1 (2026-11-10): added sc_agent_log::testing::MemoryLog behind feature `testing` (PLAN-CRIT-010). Checked sc-observability 1.5.0 source (cargo registry): no public in-memory sink exists (RecordingEventSink is #[cfg(test)]-private), so MemoryLog implements sc_observability::v2::LogSink and is registered with v2::LoggerBuilder::register_sink(SinkRegistration::typed(..)).

Plan review round 2: LogContext { session_id, position } in sc-agent-log (RSH-009), carried by GeneralLogger::write/try_write as LogEvent.correlation_id + fields; try_write for cancel/result paths (RSH-010); lost-row accounting under one Mutex with poison rationale (RBP-F015); AsRef<str> on Slug/Repo (RBP-F017); cause -> cause_detail; Display = id: message. The C2/C5 copy rows' edit column was updated; source and target columns untouched.
