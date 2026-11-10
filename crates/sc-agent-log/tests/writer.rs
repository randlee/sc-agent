/// Makes `dir` read-only; returns false (so the test skips) when the process
/// can still write there, as root can.
#[cfg(unix)]
fn make_unwritable(dir: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555)).expect("chmod");
    let probe = dir.join(".probe");
    let writable = std::fs::write(&probe, b"x").is_ok();
    let _ = std::fs::remove_file(probe);
    !writable
}

/// Audit rows lost silently: with an unwritable `logs/` directory the writer
/// drops the row but the flush still succeeds. The loss is reported once, by
/// `take_lost_rows`, so a caller can warn.
#[cfg(unix)]
#[test]
fn test_rows_the_writer_cannot_write_are_reported_once() {
    use std::os::unix::fs::PermissionsExt;
    let root = TempDir::new().expect("temp");
    let logs = root.path().join("logs");
    std::fs::create_dir(&logs).expect("logs");
    if !make_unwritable(&logs) {
        return;
    }
    let adapter = ObservabilityAdapter::open(&policy(), root.path()).expect("adapter");
    adapter.emit(event_for(0)).expect("admitted");
    adapter.emit(event_for(3)).expect("admitted");
    adapter
        .flush()
        .expect("a flush of nothing to write succeeds");
    let warning = adapter.take_lost_rows().expect("the loss is visible");
    assert!(
        warning.starts_with("2 session-log row(s) could not be written"),
        "{warning}"
    );
    assert!(warning.contains(SERVICE), "{warning}");
    assert!(adapter.take_lost_rows().is_none(), "reported once");

    // A healthy writer afterwards reports nothing new.
    std::fs::set_permissions(&logs, std::fs::Permissions::from_mode(0o700)).expect("chmod");
    adapter.emit(event_for(0)).expect("admitted");
    adapter.flush().expect("flush");
    assert!(adapter.take_lost_rows().is_none());
    assert_eq!(jsonl(root.path()).lines().count(), 1);
}

#[test]
fn test_a_healthy_writer_loses_nothing() {
    let root = TempDir::new().expect("temp");
    let adapter = ObservabilityAdapter::open(&policy(), root.path()).expect("adapter");
    adapter.emit(event_for(0)).expect("admitted");
    adapter.flush().expect("flush");
    assert!(adapter.take_lost_rows().is_none());
}

/// A short-lived process never reaches `sc-observability`'s first maintenance
/// pass (60 s after start): the adapter runs the startup pass itself.
#[test]
fn test_an_oversized_log_rotates_when_the_logger_starts() {
    let root = TempDir::new().expect("temp");
    let logs = root.path().join("logs");
    std::fs::create_dir(&logs).expect("logs");
    let active = logs.join(format!("{SERVICE}.log.jsonl"));
    // Sparse: 65 MiB, over the 64 MiB default, without writing it.
    let file = std::fs::File::create(&active).expect("file");
    file.set_len(65 * 1024 * 1024).expect("sparse");
    drop(file);

    let adapter = ObservabilityAdapter::open(&policy(), root.path()).expect("adapter");
    adapter.emit(event_for(0)).expect("row");
    adapter.flush().expect("flush");

    let rotated = logs.join(format!("{SERVICE}.log.jsonl.1"));
    assert_eq!(
        std::fs::metadata(&rotated).expect("rotated file").len(),
        65 * 1024 * 1024
    );
    let current = std::fs::read_to_string(&active).expect("active");
    assert_eq!(current.lines().count(), 1, "only the new row: {current}");
}

