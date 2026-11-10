//! close(), the last-resort report on drop, one account per shared Logger, the row cap
//! (RSH-001, QA-001/RSH-005, RSH-003).

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use common::{identity, payload};
use sc_agent_log::testing::MemoryLog;
use sc_agent_log::{
    GeneralLogger, Level, LogContext, LogError, LogKind, Loggers, MAX_ROW_BYTES, SessionLogger,
};
use sc_agent_types::{ErrorId, SessionId};
use sc_observability::v2::{
    LogEvent, LogSink, LogSinkError, Logger, LoggerBuilder, LoggerConfig, SinkRegistration,
};
use sc_observability::{
    ErrorCode, ErrorContext, Remediation, ServiceName, SinkHealth, SinkHealthState, SinkName,
};
use serde_json::{Map, json};
use tempfile::TempDir;

/// Set in a child test process to the scenario it runs.
const CHILD: &str = "SC_AGENT_LOG_TEST_CHILD";
/// The prefix of every last-resort stderr line.
const REPORT: &str = "sc-agent-log: ";

/// A default session logger whose writes all fail: its `sessions.jsonl` is a directory.
fn failing_session(root: &Path) -> Result<SessionLogger, Box<dyn std::error::Error>> {
    let identity = identity()?;
    std::fs::create_dir_all(identity.dir(root).join(SessionLogger::FILE))?;
    Ok(SessionLogger::open_default(root, identity)?)
}

/// Runs this test binary's test `name` alone with `CHILD=scenario`; returns its stderr.
fn child_stderr(name: &str, scenario: &str) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new(std::env::current_exe()?)
        .args([name, "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD, scenario)
        .output()?;
    if !output.status.success() {
        return Err(format!("child {scenario} failed: {output:?}").into());
    }
    Ok(String::from_utf8(output.stderr)?)
}

/// RSH-001: close() flushes, shuts down and returns the rows lost since the last flush.
#[test]
fn close_returns_lost_rows() {
    let root = TempDir::new().expect("temp");
    let log = failing_session(root.path()).expect("logger");
    log.try_write(SessionId::new(), Map::new())
        .expect("admitted");
    let error = log.close().expect_err("the loss is returned");
    let LogError::RowsLost { log, count, last } = &error else {
        panic!("expected rows_lost, got {error}");
    };
    assert_eq!((*log, *count), (LogKind::Session, 1));
    assert!(last.is_some());

    let healthy = GeneralLogger::open_default(root.path(), "decide").expect("general");
    healthy
        .write(Level::Info, "t", "a", LogContext::NONE, Map::new())
        .expect("row");
    healthy.close().expect("nothing lost");
}

/// RSH-001: an owned logger dropped without flush or close reports its loss as one bounded
/// last-resort stderr line; after close() a drop writes nothing.
#[test]
fn drop_without_close_reports_loss() {
    if let Some(scenario) = std::env::var_os(CHILD) {
        let root = TempDir::new().expect("temp");
        let log = failing_session(root.path()).expect("logger");
        log.try_write(SessionId::new(), Map::new())
            .expect("admitted");
        if scenario == "close" {
            assert!(log.close().is_err());
        } else {
            drop(log);
        }
        return;
    }
    let dropped = child_stderr("drop_without_close_reports_loss", "drop").expect("child");
    let lines: Vec<_> = dropped.lines().filter(|l| l.starts_with(REPORT)).collect();
    assert_eq!(lines.len(), 1, "{dropped}");
    assert!(
        lines[0].starts_with("sc-agent-log: session log closed with 1 lost row(s) (last: "),
        "{}",
        lines[0]
    );
    assert!(lines[0].len() < 300, "bounded: {}", lines[0]);

    let closed = child_stderr("drop_without_close_reports_loss", "close").expect("child");
    assert!(!closed.contains(REPORT), "{closed}");
}

/// A sink whose every write fails.
struct FailingSink {
    name: SinkName,
}

impl LogSink for FailingSink {
    fn write(&self, _event: &LogEvent) -> Result<(), LogSinkError> {
        Err(LogSinkError::Write {
            context: Box::new(ErrorContext::new(
                ErrorCode::new_static("TEST_SINK_FAILED"),
                "test sink refuses every row",
                Remediation::not_recoverable("test"),
            )),
        })
    }

    fn health(&self) -> SinkHealth {
        SinkHealth {
            name: self.name.clone(),
            state: SinkHealthState::Healthy,
            last_error: None,
        }
    }
}

fn failing_logger() -> Result<Arc<Logger>, Box<dyn std::error::Error>> {
    let mut config = LoggerConfig::default_for(ServiceName::new("failing-test")?, PathBuf::new());
    config.enable_file_sink = false;
    config.enable_console_sink = false;
    let mut builder = LoggerBuilder::new(config)?;
    builder.register_sink(SinkRegistration::typed(Arc::new(FailingSink {
        name: SinkName::new("failing")?,
    })));
    Ok(Arc::new(builder.build()?))
}

/// QA-001 / RSH-005: one row lost by a Logger that backs both loggers is reported exactly once.
#[test]
fn shared_logger_reports_once() {
    let root = TempDir::new().expect("temp");
    let logger = failing_logger().expect("logger");
    let loggers = Loggers::open(
        root.path(),
        "decide",
        identity().expect("identity"),
        Some(Arc::clone(&logger)),
        Some(Arc::clone(&logger)),
    )
    .expect("open");
    loggers
        .session
        .write(SessionId::new(), Map::new())
        .expect("admitted");
    let first = loggers.general.flush();
    let second = loggers.session.flush();
    let reports: Vec<_> = [first, second]
        .into_iter()
        .filter_map(Result::err)
        .collect();
    assert_eq!(reports.len(), 1, "{reports:?}");
    assert!(matches!(reports[0], LogError::RowsLost { count: 1, .. }));
    let last = reports[0].cause_detail().expect("latest failure");
    assert_eq!(last.id, "TEST_SINK_FAILED");
    loggers.close().expect("nothing left to report");
}

/// QA-001: loggers built from one MemoryLog share one account; a refusal is reported once.
#[test]
fn memory_log_reports_once() {
    let memory = MemoryLog::new().expect("memory");
    let general = memory.general();
    let session = memory.session(identity().expect("identity"));
    session
        .write(SessionId::new(), payload(&[("repo", json!("x"))]))
        .expect_err("reserved");
    let reports: Vec<_> = [general.flush(), session.flush()]
        .into_iter()
        .filter_map(Result::err)
        .collect();
    assert_eq!(reports.len(), 1);
    assert!(matches!(reports[0], LogError::RowsLost { count: 1, .. }));
}

/// RSH-003: a row whose fields serialize over MAX_ROW_BYTES is not written; the write returns
/// log.row_too_large and flush counts it as a lost row. A row just under the cap is written.
#[test]
fn oversized_row_refused() {
    let memory = MemoryLog::new().expect("memory");
    let general = memory.general();
    // {"blob":"<n>"} is n + 11 bytes.
    let under = payload(&[("blob", json!("x".repeat(MAX_ROW_BYTES - 11)))]);
    general
        .write(Level::Info, "t", "a", LogContext::NONE, under)
        .expect("at the cap");
    let over = payload(&[("blob", json!("x".repeat(MAX_ROW_BYTES - 10)))]);
    let error = general
        .write(Level::Info, "t", "a", LogContext::NONE, over)
        .expect_err("over the cap");
    assert!(
        matches!(error, LogError::RowTooLarge { log: LogKind::General, bytes, max }
            if bytes == MAX_ROW_BYTES + 1 && max == MAX_ROW_BYTES),
        "{error:?}"
    );
    assert_eq!(error.id(), "log.row_too_large");
    let session = memory.session(identity().expect("identity"));
    let big = payload(&[("blob", json!("x".repeat(MAX_ROW_BYTES)))]);
    assert_eq!(
        session
            .try_write(SessionId::new(), big)
            .expect_err("over")
            .id(),
        "log.row_too_large"
    );
    let LogError::RowsLost { count, last, .. } = general.flush().expect_err("counted") else {
        panic!("expected rows_lost");
    };
    assert_eq!(count, 2);
    assert_eq!(last.expect("last").id, "log.row_too_large");
    assert_eq!(
        memory.records().expect("records").len(),
        1,
        "only the row at the cap"
    );
}
