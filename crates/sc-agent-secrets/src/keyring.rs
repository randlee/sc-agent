//! Platform keyring access with a read deadline.

use crate::error::SecretsError;

/// Reads one keyring entry.
///
/// `Ok(None)` means the entry is absent. The trait is open so callers and
/// tests can inject a store. Implementations must not put secret material in
/// errors.
pub trait KeyringSource: Send + Sync {
    /// Returns the secret for `service` and `account`.
    ///
    /// # Errors
    ///
    /// Returns [`SecretsError::KeyringRead`] when the store cannot be read and
    /// [`SecretsError::KeyringTimeout`] when it does not answer in time. The
    /// error must not include secret bytes or passwords.
    fn password(&self, service: &str, account: &str) -> Result<Option<String>, SecretsError>;
}

/// Reads secrets from the platform keyring.
///
/// The service is the variable name and the account is the OS user. On macOS
/// and iOS this is the Keychain, on Windows Credential Manager, and on Linux
/// and BSD the Secret Service. Other targets have no keystore and lookups
/// miss.
///
/// The read runs on its own thread with a 15 s deadline
/// (`KEYRING_READ_TIMEOUT`); a keystore that waits for a prompt it cannot
/// show gives [`SecretsError::KeyringTimeout`] instead of hanging. Once a read
/// has timed out, every later read in the same process returns that error at
/// once, without another 15 s wait, until the stuck read finishes; so several
/// providers cost one wait, and at most one thread is stranded. A reader
/// thread that fails or panics is [`SecretsError::KeyringRead`]; the panic is
/// not propagated.
#[derive(Debug, Default, Clone, Copy)]
pub struct PlatformKeyring;

/// Longest a platform keyring read may take.
///
/// A keystore that waits for the user (a locked macOS Keychain with no way to
/// show its prompt, a Secret Service with no agent) can block a read forever.
/// A headless run must not hang on it, so the read gets a deadline and a
/// typed error ([`SecretsError::KeyringTimeout`]).
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
    /// Held for the whole of one read (start to answer or deadline), so reads
    /// run one at a time: a second caller waits for the first read's answer
    /// or deadline, then sees the stuck mark and returns at once. Without it,
    /// two concurrent callers could both pass the stuck check and strand two
    /// threads.
    in_flight: std::sync::Mutex<()>,
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
            in_flight: std::sync::Mutex::new(()),
        }
    }

    /// Whether a timed-out read is still running.
    pub(crate) fn is_stuck(&self) -> bool {
        self.stuck.load(std::sync::atomic::Ordering::SeqCst) != 0
    }

    /// Runs `read` on its own thread and waits at most `timeout`.
    ///
    /// Reads are serialized (see `in_flight`), so a caller may wait for an
    /// earlier read's deadline before its own read starts.
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
        // The guard holds no data, so a poisoned lock is still a valid one.
        let _one_at_a_time = self
            .in_flight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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

/// A keyring read that could not run to an answer and did not time out.
fn keyring_read_failed() -> SecretsError {
    SecretsError::KeyringRead {
        kind: "thread",
        cause: "the read thread failed",
        source: None,
    }
}

impl KeyringSource for PlatformKeyring {
    fn password(&self, service: &str, account: &str) -> Result<Option<String>, SecretsError> {
        let (service, account) = (service.to_owned(), account.to_owned());
        match GATE.run(KEYRING_READ_TIMEOUT, move || read_entry(&service, &account)) {
            Ok(result) => result,
            Err(DeadlineMiss::TimedOut) => Err(SecretsError::KeyringTimeout),
            Err(DeadlineMiss::Failed) => Err(keyring_read_failed()),
        }
    }
}

fn read_entry(service: &str, account: &str) -> Result<Option<String>, SecretsError> {
    let entry = match keyring::Entry::new(service, account) {
        Ok(entry) => entry,
        Err(error) => return Err(keyring_unavailable(&error)),
    };
    classify_keyring(entry.get_password())
}

pub(crate) fn classify_keyring(
    result: Result<String, keyring::Error>,
) -> Result<Option<String>, SecretsError> {
    match result {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(keyring_unavailable(&error)),
    }
}

/// Labels a keyring failure without formatting the error.
///
/// Callers pass only failures: `NoEntry` is an absent entry, handled before
/// this, and falls in the catch-all arm if it ever arrives.
///
/// `keyring::Error`'s `Display` and `Debug` can include secret bytes
/// (`BadEncoding`) or the inner platform message (`PlatformFailure`).
fn sanitized_keyring_cause(error: &keyring::Error) -> &'static str {
    match error {
        keyring::Error::PlatformFailure(_) => "platform keystore returned an error",
        keyring::Error::NoStorageAccess(_) => "keystore access was denied",
        keyring::Error::BadEncoding(_) => "stored value was not valid UTF-8",
        keyring::Error::TooLong(_, _) => "keystore rejected an attribute length",
        keyring::Error::Invalid(_, _) => "keystore rejected the service or account",
        keyring::Error::Ambiguous(_) => "keystore matched more than one entry",
        _ => "keystore returned an unrecognized error",
    }
}

/// The error for a keyring failure: this layer's label, and the keyring error whole
/// (`keyring.<kind>`; its text never renders the stored value, `BadEncoding` included).
fn keyring_unavailable(error: &keyring::Error) -> SecretsError {
    let kind = keyring_kind(error);
    SecretsError::KeyringRead {
        kind,
        cause: sanitized_keyring_cause(error),
        source: Some(sc_agent_types::ErrorDetail::foreign(
            format!("keyring.{kind}"),
            error,
        )),
    }
}

/// The keyring error variant as display-only text (`NoEntry` never arrives here).
fn keyring_kind(error: &keyring::Error) -> &'static str {
    match error {
        keyring::Error::PlatformFailure(_) => "platform_failure",
        keyring::Error::NoStorageAccess(_) => "no_storage_access",
        keyring::Error::BadEncoding(_) => "bad_encoding",
        keyring::Error::TooLong(_, _) => "too_long",
        keyring::Error::Invalid(_, _) => "invalid",
        keyring::Error::Ambiguous(_) => "ambiguous",
        _ => "unrecognized",
    }
}

/// OS user name used as the keyring account.
#[must_use]
pub(crate) fn current_account() -> String {
    whoami::username()
}

#[cfg(test)]
mod tests {
    use sc_agent_types::ErrorId;

    use super::*;

    /// REQ-SAG-0002 0.2.0: a keyring failure keeps the keyring's own error whole as its
    /// cause (`keyring.<kind>` and the keyring's message); this layer only adds its label.
    /// (Before 0.2.0 this test required the inner text to be dropped; 0.2.0 forbids
    /// replacing a lower-layer message, and a keyring read has no attached key to redact.)
    /// `BadEncoding` never renders the stored bytes, so the value does not appear.
    #[test]
    fn keyring_errors_keep_the_lower_error_whole() {
        let inner = "keystore daemon refused the request";
        let cases = [
            (
                keyring::Error::PlatformFailure(Box::new(std::io::Error::other(inner))),
                "platform keystore returned an error",
                "keyring.platform_failure",
            ),
            (
                keyring::Error::NoStorageAccess(Box::new(std::io::Error::other(inner))),
                "keystore access was denied",
                "keyring.no_storage_access",
            ),
            (
                keyring::Error::TooLong(inner.to_owned(), 8),
                "keystore rejected an attribute length",
                "keyring.too_long",
            ),
            (
                keyring::Error::Invalid("password".to_owned(), inner.to_owned()),
                "keystore rejected the service or account",
                "keyring.invalid",
            ),
        ];
        for (error, label, id) in cases {
            let rendered = error.to_string();
            let err = classify_keyring(Err(error)).expect_err("keyring failure");
            assert_eq!(err.id(), "secrets.keyring_read");
            assert!(err.message().contains(label), "{}", err.message());
            let cause = err.cause_detail().expect("the keyring error is the cause");
            assert_eq!(cause.id, id);
            assert_eq!(cause.message, rendered, "kept whole");
            assert!(rendered.contains(inner), "{rendered}");
        }
        let stored = "stored-value-that-is-not-utf8";
        let bad = keyring::Error::BadEncoding(stored.as_bytes().to_vec());
        let rendered_bad = bad.to_string();
        let err = classify_keyring(Err(bad)).expect_err("bad encoding");
        let cause = err.cause_detail().expect("cause");
        assert_eq!(cause.id, "keyring.bad_encoding");
        assert_eq!(cause.message, rendered_bad, "kept whole");
        let rendered = format!("{err} {err:?} {:?}", err.detail());
        assert!(!rendered.contains(stored), "{rendered}");
        assert_eq!(
            classify_keyring(Err(keyring::Error::NoEntry)).unwrap(),
            None
        );
    }

    use std::sync::mpsc;
    use std::time::Duration;

    /// Short deadline for tests; no test waits on a clock for its result.
    const SHORT: Duration = Duration::from_millis(20);
    /// A deadline long enough that only the read finishing ends the wait.
    const LONG: Duration = Duration::from_secs(60);

    /// Waits by yielding (never a timed pause) until the stuck mark clears.
    fn wait_until_unstuck(gate: &ReadGate) {
        let mut spins = 0_u64;
        while gate.is_stuck() {
            spins += 1;
            assert!(spins < 50_000_000, "the stuck mark never cleared");
            std::thread::yield_now();
        }
    }

    /// A read that never answers returns a typed error at the
    /// deadline instead of hanging. The blocked reader waits on a channel, so
    /// the test depends on no timed pause: it ends when the test releases the gate.
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
    }

    /// Two callers whose reads would both block start one reader thread
    /// between them: both get `TimedOut` and at most one thread is stranded.
    #[test]
    fn two_concurrent_blocked_reads_strand_one_thread() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::{Arc, Barrier, Mutex};

        let gate = Arc::new(ReadGate::new());
        let started = Arc::new(AtomicUsize::new(0));
        let (release, held) = mpsc::channel::<()>();
        let held = Arc::new(Mutex::new(held));
        let barrier = Arc::new(Barrier::new(2));
        let callers: Vec<_> = (0..2)
            .map(|index| {
                let (gate, started, held, barrier) = (
                    Arc::clone(&gate),
                    Arc::clone(&started),
                    Arc::clone(&held),
                    Arc::clone(&barrier),
                );
                std::thread::Builder::new()
                    .name(format!("caller-{index}"))
                    .spawn(move || {
                        barrier.wait();
                        gate.run(SHORT, move || {
                            started.fetch_add(1, Ordering::SeqCst);
                            let _ = held.lock().map(|receiver| receiver.recv());
                        })
                    })
                    .expect("spawn a caller")
            })
            .collect();
        for caller in callers {
            assert_eq!(caller.join().expect("caller"), Err(DeadlineMiss::TimedOut));
        }
        assert_eq!(started.load(Ordering::SeqCst), 1, "one reader thread only");
        assert!(gate.is_stuck());
        drop(release);
        wait_until_unstuck(&gate);
        assert_eq!(gate.run(LONG, || 9_u8), Ok(9));
    }

    /// A reader that panics before answering is not a timeout and leaves the
    /// gate usable.
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
        assert_eq!(error.id(), "secrets.keyring_read");
    }
}
