//! Lost-row accounting, startup retention, never-renamed session files, error causes
//! (ported from sc-jev-host tests/obs.rs:202-283, then extended).

mod common;

use std::error::Error as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use common::{events, identity, payload};
use sc_agent_log::testing::MemoryLog;
use sc_agent_log::{GeneralLogger, Level, LogContext, LogError, SessionLogger};
use sc_agent_types::{ErrorDetail, ErrorId, SessionId};
use sc_observability::v2::{
    EventError, InitError, LogEvent, LogSink, LogSinkError, Logger, LoggerBuilder, LoggerConfig,
    SinkRegistration,
};
use sc_observability::{ServiceName, SinkHealth, SinkHealthState, SinkName};
use serde_json::{Map, json};
use tempfile::TempDir;

/// 65 MiB: over sc-observability's 64 MiB default active-file limit.
const OVERSIZED: u64 = 65 * 1024 * 1024;

/// A8-14: rows the writer cannot write (unwritable session directory) are admitted, the flush
/// succeeds as a barrier, and the loss is reported once by flush() as log.rows_lost.
#[cfg(unix)]
#[test]
fn rows_lost_reported_once() {
    let root = TempDir::new().expect("temp");
    let identity = identity().expect("identity");
    let dir = identity.dir(root.path());
    std::fs::create_dir_all(&dir).expect("session dir");
    if !common::make_unwritable(&dir).expect("chmod") {
        return;
    }
    let log = SessionLogger::open_default(root.path(), identity).expect("logger");
    log.write(SessionId::new(), Map::new()).expect("admitted");
    log.write(SessionId::new(), Map::new()).expect("admitted");
    let error = log.flush().expect_err("the loss is visible");
    let LogError::RowsLost { count, last, .. } = &error else {
        panic!("expected rows_lost, got {error}");
    };
    assert_eq!(*count, 2);
    let last = last.as_ref().expect("the latest failure");
    assert!(!last.message.is_empty(), "{last:?}");
    assert_eq!(error.id(), "log.rows_lost");
    assert_eq!(error.detail().cause.as_deref(), Some(last));
    log.flush().expect("reported once");

    // A healthy writer afterwards reports nothing new.
    common::make_writable(&dir).expect("chmod");
    log.write(SessionId::new(), Map::new()).expect("admitted");
    log.flush().expect("flush");
    assert_eq!(
        events(&dir.join(SessionLogger::FILE)).expect("rows").len(),
        1
    );
}

/// A8-14: a healthy writer loses nothing.
#[test]
fn healthy_writer_loses_nothing() {
    let root = TempDir::new().expect("temp");
    let general = GeneralLogger::open_default(root.path(), "decide").expect("general");
    general
        .write(
            Level::Info,
            "sc_agent_log",
            "test.row",
            LogContext::NONE,
            Map::new(),
        )
        .expect("row");
    general.flush().expect("flush");
    let session =
        SessionLogger::open_default(root.path(), identity().expect("identity")).expect("session");
    session.write(SessionId::new(), Map::new()).expect("row");
    session.flush().expect("flush");
}

/// A8-02: a short-lived process never reaches sc-observability's first periodic pass (60 s
/// after start); GeneralLogger::open_default runs the startup pass, so an oversized general log
/// is moved aside under sc-observability's RetainedLogPolicy.
#[test]
fn general_rotates_at_start() {
    let root = TempDir::new().expect("temp");
    let logs = root.path().join("logs");
    std::fs::create_dir(&logs).expect("logs");
    let active = logs.join("decide.log.jsonl");
    // Sparse: over the 64 MiB default, without writing it.
    let file = std::fs::File::create(&active).expect("file");
    file.set_len(OVERSIZED).expect("sparse");
    drop(file);

    let log = GeneralLogger::open_default(root.path(), "decide").expect("logger");
    log.write(
        Level::Info,
        "sc_agent_log",
        "test.row",
        LogContext::NONE,
        Map::new(),
    )
    .expect("row");
    log.flush().expect("flush");

    let moved = logs.join("decide.log.jsonl.1");
    assert_eq!(
        std::fs::metadata(&moved).expect("moved file").len(),
        OVERSIZED
    );
    assert_eq!(events(&active).expect("rows").len(), 1, "only the new row");
}

/// A8-04: an over-64 MiB session file is not renamed; the new row is appended to it.
#[test]
fn session_file_never_rotates() {
    let root = TempDir::new().expect("temp");
    let identity = identity().expect("identity");
    let dir = identity.dir(root.path());
    std::fs::create_dir_all(&dir).expect("dir");
    let file_path = dir.join(SessionLogger::FILE);
    let file = std::fs::File::create(&file_path).expect("file");
    file.set_len(OVERSIZED).expect("sparse");
    drop(file);

    let log = SessionLogger::open_default(root.path(), identity).expect("logger");
    log.write(SessionId::new(), Map::new()).expect("row");
    log.flush().expect("flush");
    drop(log);

    let names: Vec<_> = std::fs::read_dir(&dir)
        .expect("dir")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    assert_eq!(names, [SessionLogger::FILE], "nothing renamed or added");
    assert!(std::fs::metadata(&file_path).expect("file").len() > OVERSIZED);
}

/// A8-04: session files older than sc-observability's 7-day retention age (mtime 8 days back)
/// are still present after a new SessionLogger writes and flushes in the same directory.
#[test]
fn old_session_log_survives() {
    let root = TempDir::new().expect("temp");
    let identity = identity().expect("identity");
    let dir = identity.dir(root.path());
    std::fs::create_dir_all(&dir).expect("dir");
    let eight_days_ago = SystemTime::now() - Duration::from_secs(8 * 86_400);
    let active = dir.join(SessionLogger::FILE);
    // `sessions.jsonl.1` is the name the default policy would delete by age.
    let numbered = dir.join(format!("{}.1", SessionLogger::FILE));
    for path in [&active, &numbered] {
        std::fs::write(path, "{}\n").expect("old file");
        let file = std::fs::File::options()
            .write(true)
            .open(path)
            .expect("open");
        file.set_modified(eight_days_ago).expect("mtime");
    }

    let log = SessionLogger::open_default(root.path(), identity).expect("logger");
    log.write(SessionId::new(), Map::new()).expect("row");
    log.flush().expect("flush");
    drop(log);

    assert!(numbered.exists(), "the 8-day-old numbered file survives");
    let text = std::fs::read_to_string(&active).expect("active");
    assert_eq!(text.lines().count(), 2, "old row kept, new row appended");
}

/// A8-14: a payload key reserved for identity or context is refused, counted and reported once.
#[test]
fn reserved_field_refused_and_counted() {
    let memory = MemoryLog::new().expect("memory");
    let session = memory.session(identity().expect("identity"));
    let error = session
        .write(SessionId::new(), payload(&[("repo", json!("x"))]))
        .expect_err("reserved");
    assert!(matches!(&error, LogError::ReservedField { key } if key == "repo"));
    assert_eq!(error.id(), "log.reserved_field");
    let error = session.flush().expect_err("counted");
    let LogError::RowsLost { count, last, .. } = error else {
        panic!("expected rows_lost, got {error}");
    };
    assert_eq!(count, 1);
    assert_eq!(last.expect("last").id, "log.reserved_field");
    session.flush().expect("reported once");

    let general = memory.general();
    for key in GeneralLogger::CONTEXT_KEYS {
        general
            .write(
                Level::Info,
                "t",
                "a",
                LogContext::NONE,
                payload(&[(key, json!(1))]),
            )
            .expect_err("reserved");
    }
    assert!(matches!(
        general.flush(),
        Err(LogError::RowsLost { count: 2, .. })
    ));
    assert!(
        memory.records().expect("records").is_empty(),
        "nothing written"
    );
}

/// A8-14: threads refuse rows while another thread flushes; every refusal is reported exactly
/// once (all lost-row state is under one Mutex).
#[test]
fn concurrent_refusals_counted_once() {
    const THREADS: u64 = 4;
    const PER_THREAD: u64 = 500;
    let memory = MemoryLog::new().expect("memory");
    let general = memory.general();
    let reported = AtomicU64::new(0);
    let done = AtomicBool::new(false);
    let add = |result: Result<(), LogError>| match result {
        Ok(()) => {}
        Err(LogError::RowsLost { count, .. }) => {
            reported.fetch_add(count, Ordering::SeqCst);
        }
        Err(other) => panic!("unexpected {other}"),
    };
    std::thread::scope(|scope| {
        let refusers: Vec<_> = (0..THREADS)
            .map(|_| {
                let general = general.clone();
                scope.spawn(move || {
                    for _ in 0..PER_THREAD {
                        let row = payload(&[("position", json!(0))]);
                        assert!(
                            general
                                .write(Level::Info, "t", "a", LogContext::NONE, row)
                                .is_err()
                        );
                    }
                })
            })
            .collect();
        scope.spawn(|| {
            while !done.load(Ordering::SeqCst) {
                add(general.flush());
            }
        });
        for refuser in refusers {
            refuser.join().expect("refuser");
        }
        done.store(true, Ordering::SeqCst);
    });
    add(general.flush());
    assert_eq!(reported.load(Ordering::SeqCst), THREADS * PER_THREAD);
}

/// A sink that blocks every write until its gate's sender is dropped.
struct GateSink {
    gate: Mutex<Receiver<()>>,
    name: SinkName,
}

impl LogSink for GateSink {
    fn write(&self, _event: &LogEvent) -> Result<(), LogSinkError> {
        // Returns when the sender is dropped; later writes return at once.
        let _ = self
            .gate
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .recv();
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

/// A logger whose one sink blocks until the returned sender is dropped; queue capacity 1.
fn gated_logger() -> Result<(Arc<Logger>, Sender<()>), Box<dyn std::error::Error>> {
    let (open, gate) = channel();
    let sink = GateSink {
        gate: Mutex::new(gate),
        name: SinkName::new("gate")?,
    };
    let mut config = LoggerConfig::default_for(ServiceName::new("gate-test")?, PathBuf::new());
    config.enable_file_sink = false;
    config.enable_console_sink = false;
    config.queue_capacity = 1;
    let mut builder = LoggerBuilder::new(config)?;
    builder.register_sink(SinkRegistration::typed(Arc::new(sink)));
    Ok((Arc::new(builder.build()?), open))
}

/// Calls `try_write` until it fails, while the writer thread is stuck in the gated sink;
/// `None` if it never fails. Returning at all proves try_write never blocks on a full queue.
fn first_refusal(mut try_write: impl FnMut() -> Result<(), LogError>) -> Option<LogError> {
    // The writer holds at most one batch (64 rows) plus the queue (1 row) before a refusal.
    (0..10_000).find_map(|_| try_write().err())
}

/// A8-21: try_write on a full queue returns at once with an Err, counted as a lost row and
/// returned once by flush() as log.rows_lost.
#[test]
fn try_write_never_blocks() {
    let (logger, open) = gated_logger().expect("gated logger");
    let general = GeneralLogger::injected(Arc::clone(&logger));
    let error =
        first_refusal(|| general.try_write(Level::Info, "t", "a", LogContext::NONE, Map::new()))
            .expect("the queue fills");
    assert_eq!(error.id(), "log.rejected");
    let session = SessionLogger::injected(Arc::clone(&logger), identity().expect("identity"));
    let session_error =
        first_refusal(|| session.try_write(SessionId::new(), Map::new())).expect("the queue fills");
    assert_eq!(session_error.id(), "log.rejected");

    drop(open);
    let LogError::RowsLost { count, last, .. } = general.flush().expect_err("counted") else {
        panic!("expected rows_lost");
    };
    assert_eq!(count, 1);
    let last = last.expect("last");
    assert_eq!(last.id, "log.rejected");
    assert_eq!(
        last.cause.expect("queue cause").id,
        sc_observability::error_codes::LOGGER_QUEUE_FULL.as_str()
    );
    general.flush().expect("reported once");
    assert!(matches!(
        session.flush(),
        Err(LogError::RowsLost { count: 1, .. })
    ));
    session.flush().expect("reported once");
}

/// A8-15: an sc-observability failure keeps its code and message as source() and ErrorDetail.
#[test]
fn log_error_keeps_lower_layer_cause() {
    // sc-observability refuses a write on a logger the caller shut down.
    let memory = MemoryLog::new().expect("memory");
    let logger = memory.logger();
    let general = GeneralLogger::injected(Arc::clone(&logger));
    logger.shutdown().expect("shutdown");
    let error = general
        .write(Level::Info, "t", "a", LogContext::NONE, Map::new())
        .expect_err("refused after shutdown");
    assert_eq!(error.id(), "log.rejected");
    let source = error
        .source()
        .and_then(|source| source.downcast_ref::<EventError>())
        .expect("the EventError is the source");
    let cause = error.detail().cause.expect("cause detail");
    assert_eq!(cause.id, source.diagnostic().code.as_str());
    assert_eq!(cause.message, source.to_string());
    assert!(!cause.message.is_empty());

    // sc-observability refuses a service name.
    let root = TempDir::new().expect("temp");
    let error = GeneralLogger::open_default(root.path(), "not a name").expect_err("refused");
    assert_eq!(error.id(), "log.invalid_name");
    let cause = error.detail().cause.expect("cause detail");
    let source = error
        .source()
        .and_then(|source| source.downcast_ref::<ErrorDetail>())
        .expect("sc-observability's refusal is the source");
    assert_eq!(source, cause.as_ref());
    assert_eq!(cause.id, "SC_OBSERVABILITY_TYPES_VALUE_VALIDATION_FAILED");
    assert_eq!(cause.message, "identifier must match [A-Za-z0-9._-]+");
}

/// A8-15: an io failure under an unwritable <log_root> reaches the LogError through the
/// sc-observability error that wraps it, with its id and message in source() and ErrorDetail.
#[cfg(unix)]
#[test]
fn log_error_keeps_io_cause() {
    let root = TempDir::new().expect("temp");
    let logs = root.path().join("logs");
    std::fs::create_dir(&logs).expect("logs");
    let file = std::fs::File::create(logs.join("decide.log.jsonl")).expect("file");
    file.set_len(OVERSIZED).expect("sparse");
    drop(file);
    if !common::make_unwritable(&logs).expect("chmod") {
        return;
    }
    let error = GeneralLogger::open_default(root.path(), "decide").expect_err("cannot move");
    common::make_writable(&logs).expect("chmod");
    assert_eq!(error.id(), "log.open_sink");
    assert!(
        error
            .source()
            .is_some_and(|source| source.is::<InitError>())
    );

    // Walk the source chain to the io error.
    let mut level = error.source();
    let mut io = None;
    while let Some(current) = level {
        if let Some(found) = current.downcast_ref::<std::io::Error>() {
            io = Some(found);
            break;
        }
        level = current.source();
    }
    let io = io.expect("the io error is in the source chain");
    assert_eq!(io.kind(), std::io::ErrorKind::PermissionDenied);

    // The same io error is a level of the ErrorDetail chain, by id and whole message: a
    // foreign source level (`ErrorDetail::foreign` names nested sources `foreign.source`).
    let mut detail = error.detail().cause;
    let mut io_level = None;
    while let Some(current) = detail {
        if current.message == io.to_string() {
            io_level = Some((current.id.clone(), current.message.clone()));
        }
        detail = current.cause;
    }
    let (id, message) = io_level.expect("the io error is a level of the ErrorDetail chain");
    assert_eq!(id, "foreign.source");
    assert_eq!(message, io.to_string());
}

/// A8-15: a LogError that wraps an io error directly names it `io.<ErrorKind>` in its
/// ErrorDetail, with the io message whole.
#[test]
fn a_direct_io_cause_is_named_by_its_kind() {
    let error = sc_agent_log::LogError::GitUnavailable {
        source: std::io::Error::new(std::io::ErrorKind::NotFound, "git: command not found"),
    };
    let cause = error.cause_detail().expect("io cause");
    assert_eq!(cause.id, "io.NotFound");
    assert_eq!(cause.message, "git: command not found");
}
