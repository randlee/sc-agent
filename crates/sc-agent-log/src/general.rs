/// Prepares the log directory and active file of `config` before a logger
/// starts on them:
///
/// 1. `<log_root>/logs` is created owner-only, if missing;
/// 2. the active file is created owner-only (`0600`), if missing (an
///    existing file is not opened), because
///    `sc-observability` would create it under the process umask;
/// 3. one retention pass runs now (rotate an active file over
///    `rotation_max_bytes`, prune rotated files by count and age).
///    `sc-observability` runs its first pass only 60 s after the logger
///    starts, so a short-lived process (a CLI call) would otherwise never
///    rotate; this is its public startup pass (`JsonlFileSink::open`);
/// 4. the active and rotated files are made owner-only (a rotation creates a
///    new active file under the umask).
///
/// # Errors
///
/// The `io::Error` of the directory or file create. The retention pass and
/// the mode changes are best effort and never fail this call.
pub fn prepare_log_files(config: &LoggerConfig) -> std::io::Result<()> {
    let dir = logs_dir(&config.log_root);
    create_private_dirs(&dir)?;
    let name = file_name(&config.service_name);
    let active = dir.join(&name);
    // `create_new` only: it never opens an existing file, so a FIFO or a
    // device the user pointed the log at cannot block this call.
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(&active) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    // Startup retention pass; a failure resurfaces as a lost row.
    let _ = JsonlFileSink::open(active, config.retained_log_policy);
    harden_log_files(&dir, &name);
    Ok(())
}
