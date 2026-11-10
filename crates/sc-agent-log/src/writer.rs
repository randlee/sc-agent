/// Owns one running logger. Dropping it flushes and shuts the writer down.
///
/// Shutdown runs exactly once, when the last [`Arc`] goes away, so there is
/// no reference-count race between concurrent drops.
///
/// # Fork
///
/// The writer is a thread, and a forked child has only the forking thread. A
/// logger started by another process (`pid` differs) is never used or shut
/// down here: its channel is dead and a send on it can abort the child. The
/// registry skips it, so the child starts a writer of its own, and dropping
/// it leaks the dead one instead of touching it.
struct LoggerInner {
    logger: Arc<Logger>,
    /// The process that started the writer thread.
    pid: u32,
    /// Rows already reported as lost (see [`LoggerInner::take_lost_rows`]).
    reported_lost: AtomicU64,
    /// `<log_root>/logs`, to keep its files owner-only.
    logs_dir: PathBuf,
    /// The active file name; rotated files are `<name>.<n>`.
    file_name: String,
}

impl LoggerInner {
    fn new(logger: Logger, log_root: &Path, service: &ServiceName) -> Self {
        Self {
            logger: Arc::new(logger),
            pid: std::process::id(),
            reported_lost: AtomicU64::new(0),
            logs_dir: logs_dir(log_root),
            file_name: file_name(service),
        }
    }

    /// Whether this process started the writer thread.
    fn is_local(&self) -> bool {
        self.pid == std::process::id()
    }

    /// Flushes admitted events, keeps the files owner-only (rotation makes a
    /// new file under the process umask), and maps a failure to `IO_FAILED`.
    fn flush(&self, label: &str) -> Result<(), JevError> {
        let flushed = blocking(&self.logger, |logger| {
            logger
                .flush()
                .map_err(|error| error.diagnostic().message.clone())
        })
        .unwrap_or_else(|| Err("the flush thread could not run".to_owned()))
        .map_err(|message| {
            JevError::io_failed_with(
                format!("{label} flush ({message})"),
                std::io::ErrorKind::Other,
                Some("Check that the log root is writable.".to_owned()),
            )
        });
        harden_log_files(&self.logs_dir, &self.file_name);
        flushed
    }

    /// Text for the rows the writer could not write since the last call, or
    /// `None` when none were lost.
    ///
    /// The writer counts a row it fails to write (`dropped_events_total`) but
    /// a flush still succeeds, so a read-only directory or a full disk loses
    /// rows with no error. Call after [`Self::flush`]: the flush is a barrier,
    /// so every admitted row is counted by then.
    fn take_lost_rows(&self, label: &str) -> Option<String> {
        let health = self.logger.health();
        let total = health.dropped_events_total;
        let seen = self.reported_lost.fetch_max(total, Ordering::AcqRel);
        let lost = total.saturating_sub(seen);
        if lost == 0 {
            return None;
        }
        let cause = health
            .last_error
            .or(health.last_writer_error)
            .map_or_else(String::new, |summary| format!(" ({})", summary.message));
        Some(format!(
            "{lost} {label} row(s) could not be written to {}{cause}",
            self.logs_dir.join(&self.file_name).display()
        ))
    }
}

impl Drop for LoggerInner {
    fn drop(&mut self) {
        if !self.is_local() {
            // Forked from the process that owns the writer: leak it. The
            // extra strong count is never released, so the dead logger is
            // never dropped.
            std::mem::forget(Arc::clone(&self.logger));
            return;
        }
        // Best effort: the process is done with this writer and a failure
        // here has no caller left to report to.
        let _ = blocking(&self.logger, |logger| {
            let _ = logger.flush();
            let _ = logger.shutdown();
        });
    }
}
/// The file-only logger configuration at `level` for `service`.
fn logger_config(service: ServiceName, log_root: &Path, level: Level) -> LoggerConfig {
    let mut config = LoggerConfig::default_for(service, log_root.to_path_buf());
    // `LevelFilter` is not re-exported by `sc-observability` and
    // `sc-observability-types` is not a direct dependency, so the filter is
    // built from the variant name through serde, like the envelope ids. A
    // miss keeps the logger's default (`Info`).
    config.level =
        serde_json::from_value(Value::String(format!("{level:?}"))).unwrap_or(config.level);
    config.enable_console_sink = false;
    config.enable_file_sink = true;
    config
}

fn start_logger(
    service: ServiceName,
    log_root: &Path,
    flag: &str,
    level: Level,
    label: &str,
) -> Result<Logger, JevError> {
    let config = logger_config(service, log_root, level);
    // Best effort: a directory that cannot be prepared is reported when the
    // writer fails to write (`take_lost_rows`), not by refusing to start.
    let _ = prepare_log_files(&config);
    LoggerBuilder::new(config)
        .and_then(LoggerBuilder::build)
        .map_err(|error| {
            JevError::config_invalid_with(
                format!(
                    "the {label} could not start at {} ({})",
                    log_root.display(),
                    error.diagnostic().message
                ),
                format!("Pass a writable {flag} and retry."),
            )
        })
}
