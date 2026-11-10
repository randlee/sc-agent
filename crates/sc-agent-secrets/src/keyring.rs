/// Reads one keyring entry.
///
/// `Ok(None)` means the entry is absent. The trait is an open extension point
/// (ADR-SAG-0020) so tests can inject a store. It is intentionally not sealed.
/// Implementations must not put secret material in errors. A store that cannot
/// be read returns [`JevError::io_failed_with`] (`IO_FAILED`) with a
/// sanitized cause in `what`.
pub trait KeyringSource: Send + Sync {
    /// Returns the secret for `service` and `account`.
    ///
    /// # Errors
    ///
    /// Returns [`JevError::IoFailed`] when the store cannot be read.
    /// The error must not include secret bytes or passwords.
    fn password(&self, service: &str, account: &str) -> Result<Option<String>, JevError>;
}
/// Reads secrets from the platform keyring.
///
/// On macOS and iOS this is the Keychain. On Windows it is Credential Manager.
/// On Linux and BSD it is the Secret Service. Other targets have no keystore
/// and lookups miss.
///
/// The read runs on its own thread with a 15 s deadline
/// (`KEYRING_READ_TIMEOUT`); a keystore that waits for a prompt it cannot
/// show gives an `IO_FAILED` (`other`, not retryable: the blocked read does
/// not clear until the user unlocks the keystore) instead of hanging. Once a
/// read has timed out, every later read in the same process returns that error
/// at once, without another 15 s wait, until the stuck read finishes; so
/// several backends cost one wait, and at most one thread is stranded. A
/// reader thread that fails or panics is the same non-retryable `IO_FAILED`.
/// This method does not panic: the reader runs on its own thread, so a panic
/// there (such as a poisoned process-global keyring builder lock) is reported
/// as that `IO_FAILED`, not propagated.
#[derive(Debug, Default, Clone, Copy)]
pub struct PlatformKeyring;

/// Longest a platform keyring read may take.
///
/// A keystore that waits for the user (a locked macOS Keychain with no way to
/// show its prompt, a Secret Service with no agent) can block a read forever.
/// A headless run must not hang on it, so the read gets a deadline and a
/// typed, non-retryable error (`IO_FAILED`, `other`).
pub(crate) const KEYRING_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// Why a bounded read returned nothing.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DeadlineMiss {
    /// The deadline passed with the read still running, or an earlier read is
    /// still stuck.
    TimedOut,
    /// The reader thread ended without an answer (it panicked), or could not
    /// be started.
    Failed,
}

/// Runs reads on their own threads with a deadline, and remembers one stuck
/// read.
///
/// The state is one number: the id of the read that timed out and has not
/// finished (0 for none). It is set on the timeout path and cleared only by
/// that same reader finishing (a drop guard, so a panic clears it too). While
/// it is set, every `run` returns [`DeadlineMiss::TimedOut`] at once, without
/// starting a thread: several backends cost one wait and at most one thread is
/// stranded. No clock is compared. Tests build their own gate, so none shares
/// a process global.
pub(crate) struct ReadGate {
    stuck: std::sync::Arc<std::sync::atomic::AtomicU64>,
    next_id: std::sync::atomic::AtomicU64,
}

/// Clears the gate's stuck mark if (and only if) it names this reader.
struct Finished {
    id: u64,
    finished: std::sync::Arc<std::sync::atomic::AtomicBool>,
    stuck: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl Drop for Finished {
    fn drop(&mut self) {
        use std::sync::atomic::Ordering;
        self.finished.store(true, Ordering::SeqCst);
        let _ = self
            .stuck
            .compare_exchange(self.id, 0, Ordering::SeqCst, Ordering::SeqCst);
    }
}

impl ReadGate {
    pub(crate) fn new() -> Self {
        Self {
            stuck: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            next_id: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Whether a timed-out read is still running.
    pub(crate) fn is_stuck(&self) -> bool {
        self.stuck.load(std::sync::atomic::Ordering::SeqCst) != 0
    }

    /// Runs `read` on its own thread and waits at most `timeout`.
    ///
    /// On timeout the thread is left to finish (or stay blocked) on its own;
    /// it only reads the keystore and holds no state, so abandoning it is
    /// safe and the process can still exit. A thread that cannot be started,
    /// or that panics before answering, is [`DeadlineMiss::Failed`].
    pub(crate) fn run<T: Send + 'static>(
        &self,
        timeout: std::time::Duration,
        read: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T, DeadlineMiss> {
        use std::sync::atomic::{AtomicBool, Ordering};
        if self.is_stuck() {
            return Err(DeadlineMiss::TimedOut);
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let finished = std::sync::Arc::new(AtomicBool::new(false));
        let guard = Finished {
            id,
            finished: std::sync::Arc::clone(&finished),
            stuck: std::sync::Arc::clone(&self.stuck),
        };
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("sc-keyring-read".to_owned())
            .spawn(move || {
                let _guard = guard;
                let _ = sender.send(read());
            })
            .map_err(|_| DeadlineMiss::Failed)?;
        receiver.recv_timeout(timeout).map_err(|error| match error {
            std::sync::mpsc::RecvTimeoutError::Timeout => {
                self.stuck.store(id, Ordering::SeqCst);
                // The reader may have finished between the timeout and the
                // store above; if so it will not clear the mark again.
                if finished.load(Ordering::SeqCst) {
                    let _ = self
                        .stuck
                        .compare_exchange(id, 0, Ordering::SeqCst, Ordering::SeqCst);
                }
                DeadlineMiss::TimedOut
            }
            std::sync::mpsc::RecvTimeoutError::Disconnected => DeadlineMiss::Failed,
        })
    }
}

/// The process-wide gate of [`PlatformKeyring`].
static GATE: std::sync::LazyLock<ReadGate> = std::sync::LazyLock::new(ReadGate::new);

/// A keyring read that could not run to an answer and did not time out:
/// not retryable, since repeating it fails the same way.
fn keyring_read_failed() -> JevError {
    JevError::io_failed_with(
        format!("{KEYRING_UNREADABLE} (the read thread failed)"),
        std::io::ErrorKind::Other,
        Some(KEYRING_RECOVERY.to_owned()),
    )
}

fn keyring_timed_out() -> JevError {
    // `other`, not `timed_out`: an identical retry meets the same stuck
    // keystore, so this is not retryable until the user unlocks it.
    JevError::io_failed_with(
        format!("{KEYRING_UNREADABLE} (it did not answer in time)"),
        std::io::ErrorKind::Other,
        Some("Unlock or allow the platform keystore for this OS user, then run again (a retry blocks again until the keystore is reachable), or provide the key in the process environment or the repo-root .env file instead.".to_owned()),
    )
}

impl KeyringSource for PlatformKeyring {
    fn password(&self, service: &str, account: &str) -> Result<Option<String>, JevError> {
        let (service, account) = (service.to_owned(), account.to_owned());
        match GATE.run(KEYRING_READ_TIMEOUT, move || read_entry(&service, &account)) {
            Ok(result) => result,
            Err(DeadlineMiss::TimedOut) => Err(keyring_timed_out()),
            Err(DeadlineMiss::Failed) => Err(keyring_read_failed()),
        }
    }
}

fn read_entry(service: &str, account: &str) -> Result<Option<String>, JevError> {
    let entry = match keyring::Entry::new(service, account) {
        Ok(entry) => entry,
        Err(error) => {
            return Err(keyring_unavailable(
                sanitized_keyring_cause(&error),
                keyring_io_kind(&error),
            ));
        }
    };
    classify_keyring(entry.get_password())
}

pub(crate) fn classify_keyring(
    result: Result<String, keyring::Error>,
) -> Result<Option<String>, JevError> {
    match result {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(keyring_unavailable(
            sanitized_keyring_cause(&error),
            keyring_io_kind(&error),
        )),
    }
}

/// Labels a keyring failure without formatting the error.
///
/// `keyring::Error`'s `Display` and `Debug` can include secret bytes
/// (`BadEncoding`) or the inner platform message (`PlatformFailure`).
fn sanitized_keyring_cause(error: &keyring::Error) -> &'static str {
    match error {
        keyring::Error::PlatformFailure(_) => "platform keystore returned an error",
        keyring::Error::NoStorageAccess(_) => "keystore access was denied",
        keyring::Error::NoEntry => "entry is absent",
        keyring::Error::BadEncoding(_) => "stored value was not valid UTF-8",
        keyring::Error::TooLong(_, _) => "keystore rejected an attribute length",
        keyring::Error::Invalid(_, _) => "keystore rejected the service or account",
        keyring::Error::Ambiguous(_) => "keystore matched more than one entry",
        _ => "keystore returned an unrecognized error",
    }
}

fn keyring_unavailable(cause: &str, kind: std::io::ErrorKind) -> JevError {
    JevError::io_failed_with(
        format!("{KEYRING_UNREADABLE} ({cause})"),
        kind,
        Some(KEYRING_RECOVERY.to_owned()),
    )
}

/// The I/O class a keyring failure is reported as; the platform message is
/// never kept.
fn keyring_io_kind(error: &keyring::Error) -> std::io::ErrorKind {
    match error {
        keyring::Error::NoStorageAccess(_) => std::io::ErrorKind::PermissionDenied,
        keyring::Error::BadEncoding(_) => std::io::ErrorKind::InvalidData,
        _ => std::io::ErrorKind::Other,
    }
}

/// OS user name used as the keyring account.
#[must_use]
pub(crate) fn current_account() -> String {
    whoami::username()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyring_errors_drop_secret_bytes() {
        let secret = "super-secret-value";
        let cases = [
            (
                keyring::Error::BadEncoding(secret.as_bytes().to_vec()),
                "stored value was not valid UTF-8",
            ),
            (
                keyring::Error::PlatformFailure(Box::new(std::io::Error::other(secret))),
                "platform keystore returned an error",
            ),
            (
                keyring::Error::NoStorageAccess(Box::new(std::io::Error::other(secret))),
                "keystore access was denied",
            ),
            (
                keyring::Error::TooLong(secret.to_owned(), 8),
                "keystore rejected an attribute length",
            ),
            (
                keyring::Error::Invalid("password".to_owned(), secret.to_owned()),
                "keystore rejected the service or account",
            ),
        ];
        for (error, cause) in cases {
            let err = classify_keyring(Err(error)).expect_err("keyring failure");
            assert_eq!(err.code(), "IO_FAILED");
            let message = err.message();
            assert!(message.contains(KEYRING_UNREADABLE), "{message}");
            assert!(message.contains(cause), "{message}");
            let action = err.suggested_action();
            assert!(action.contains(KEYRING_RECOVERY), "{action}");
            assert!(!action.contains("check.toml"), "{action}");
            assert_secret_absent(&err, secret);
        }
        assert_eq!(
            classify_keyring(Err(keyring::Error::NoEntry)).unwrap(),
            None
        );
    }

    fn assert_secret_absent(error: &JevError, secret: &str) {
        let rendered = format!("{error} {error:?} {}", error.message());
        assert!(!rendered.contains(secret), "{rendered}");
    }

    use std::sync::mpsc;
    use std::time::Duration;

    /// Short deadline for tests; no test waits on a clock for its result.
    const SHORT: Duration = Duration::from_millis(20);
    /// A deadline long enough that only the read finishing ends the wait.
    const LONG: Duration = Duration::from_secs(60);

    /// Waits (by yielding, no sleep) until the gate's stuck mark clears.
    fn wait_until_unstuck(gate: &ReadGate) {
        let mut spins = 0_u64;
        while gate.is_stuck() {
            spins += 1;
            assert!(spins < 50_000_000, "the stuck mark never cleared");
            std::thread::yield_now();
        }
    }

    /// A read that never answers returns a typed, non-retryable error at the
    /// deadline instead of hanging. The blocked reader waits on a channel, so
    /// the test depends on no sleep: it ends when the test releases the gate.
    #[test]
    fn a_blocked_keyring_read_times_out_with_a_typed_error() {
        let gate = ReadGate::new();
        let (release, held) = mpsc::channel::<()>();
        let blocked = gate.run(SHORT, move || {
            let _ = held.recv();
            1_u8
        });
        assert_eq!(blocked, Err(DeadlineMiss::TimedOut));
        assert!(gate.is_stuck());
        // While it is stuck a later read fails at once and never runs.
        let ran = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = std::sync::Arc::clone(&ran);
        let later = gate.run(LONG, move || {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        assert_eq!(later, Err(DeadlineMiss::TimedOut));
        assert!(!ran.load(std::sync::atomic::Ordering::SeqCst));
        // Releasing the stuck reader clears the mark, and reads work again.
        drop(release);
        wait_until_unstuck(&gate);
        assert_eq!(gate.run(LONG, || 7_u8), Ok(7));
        let error = keyring_timed_out();
        assert_eq!(error.code(), "IO_FAILED");
        assert!(!error.retryable(), "a retry meets the same stuck keystore");
        assert!(error.suggested_action().contains("process environment"));
        assert!(error.suggested_action().contains("blocks again"));
    }

    /// A reader that panics before answering is not a timeout, is not
    /// retryable, and leaves the gate usable.
    #[test]
    fn a_panicking_keyring_read_is_not_a_timeout_and_the_next_read_works() {
        let gate = ReadGate::new();
        let result = gate.run(LONG, || -> u8 {
            panic!("keystore reader failed (expected in this test)")
        });
        assert_eq!(result, Err(DeadlineMiss::Failed));
        assert!(!gate.is_stuck());
        assert_eq!(gate.run(LONG, || 3_u8), Ok(3));
        let error = keyring_read_failed();
        assert_eq!(error.code(), "IO_FAILED");
        assert!(!error.retryable());
    }

    /// A quick read that was already running when an older one was declared
    /// stuck does not clear the older one's mark when it finishes.
    #[test]
    fn a_quick_read_finishing_does_not_clear_an_older_stuck_one() {
        let gate = std::sync::Arc::new(ReadGate::new());
        let (started_tx, started_rx) = mpsc::channel::<()>();
        let (finish_quick, quick_held) = mpsc::channel::<()>();
        let quick_gate = std::sync::Arc::clone(&gate);
        // The quick read starts first and waits to be told to finish.
        let quick = std::thread::spawn(move || {
            quick_gate.run(LONG, move || {
                let _ = started_tx.send(());
                let _ = quick_held.recv();
                5_u8
            })
        });
        started_rx.recv().expect("the quick read started");
        // Another read, started later, times out and is stuck.
        let (release_stuck, stuck_held) = mpsc::channel::<()>();
        let stuck = gate.run(SHORT, move || {
            let _ = stuck_held.recv();
        });
        assert_eq!(stuck, Err(DeadlineMiss::TimedOut));
        assert!(gate.is_stuck());
        // The quick read finishes now; the stuck mark is not its to clear.
        finish_quick.send(()).expect("release the quick read");
        assert_eq!(quick.join().expect("quick thread"), Ok(5));
        assert!(gate.is_stuck(), "the stuck read's mark must survive");
        drop(release_stuck);
        wait_until_unstuck(&gate);
    }
}
