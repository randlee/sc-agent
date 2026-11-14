//! Platform keyring access with a read deadline.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use sc_agent_types::{CancelToken, Clock, Wait};

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
#[derive(Debug, Clone, Copy)]
pub struct PlatformKeyring {
    _private: (),
}

impl PlatformKeyring {
    /// Constructs the real OS keyring backend.
    ///
    /// Test builds with `test-keyring-guard`, and Python test runs setting
    /// `SC_AGENT_TEST_TRAP_PLATFORM_KEYRING`, refuse construction before any
    /// OS keyring call can occur.
    #[must_use]
    #[allow(clippy::panic)] // Intentional test tripwire; it must fail before OS access.
    pub fn new() -> Self {
        #[cfg(feature = "test-keyring-guard")]
        panic!("PlatformKeyring construction is forbidden in guarded test builds");

        #[cfg(not(feature = "test-keyring-guard"))]
        {
            assert_ne!(
                std::env::var_os("SC_AGENT_TEST_TRAP_PLATFORM_KEYRING").as_deref(),
                Some(std::ffi::OsStr::new("1")),
                "PlatformKeyring construction is forbidden during tests"
            );
            Self { _private: () }
        }
    }
}

impl Default for PlatformKeyring {
    fn default() -> Self {
        Self::new()
    }
}

/// Defers constructing the OS backend until a secret lookup actually needs it.
#[derive(Debug, Default, Clone, Copy)]
pub struct DeferredPlatformKeyring;

impl KeyringSource for DeferredPlatformKeyring {
    fn password(&self, service: &str, account: &str) -> Result<Option<String>, SecretsError> {
        PlatformKeyring::new().password(service, account)
    }
}

/// Longest a platform keyring read may take.
///
/// A keystore that waits for the user (a locked macOS Keychain with no way to
/// show its prompt, a Secret Service with no agent) can block a read forever.
/// A headless run must not hang on it, so the read gets a deadline and a
/// typed error ([`SecretsError::KeyringTimeout`]).
pub(crate) const KEYRING_READ_TIMEOUT: Duration = Duration::from_secs(15);

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
/// stranded. The deadline is waited for on the caller's [`Clock`], so a test
/// moves it with a manual clock. Tests build their own gate, so none shares a
/// process global.
pub(crate) struct ReadGate {
    stuck: Arc<AtomicU64>,
    next_id: AtomicU64,
    /// Held for the whole of one read (start to answer or deadline), so reads
    /// run one at a time: a second caller waits for the first read's answer
    /// or deadline, then sees the stuck mark and returns at once. Without it,
    /// two concurrent callers could both pass the stuck check and strand two
    /// threads.
    in_flight: Mutex<()>,
    /// The `finished` latch of the read that is stuck now, so a test can wait
    /// (bounded) for it to clear instead of polling.
    #[cfg(test)]
    stuck_read: Mutex<Option<CancelToken>>,
}

/// The answer slot of one read and the latch that says the reader has ended.
struct Answer<T> {
    value: Mutex<Option<T>>,
    /// Cancelled once the reader ended (answer stored, or panicked): the
    /// caller's deadline wait ends on it.
    finished: CancelToken,
}

/// Run by the reader thread when it ends, even by a panic: clears the gate's
/// stuck mark if (and only if) it names this reader, then fires `finished`.
struct Finished {
    id: u64,
    finished: CancelToken,
    stuck: Arc<AtomicU64>,
}

impl Drop for Finished {
    fn drop(&mut self) {
        let _ = self
            .stuck
            .compare_exchange(self.id, 0, Ordering::SeqCst, Ordering::SeqCst);
        self.finished.cancel();
    }
}

impl ReadGate {
    pub(crate) fn new() -> Self {
        Self {
            stuck: Arc::new(AtomicU64::new(0)),
            next_id: AtomicU64::new(1),
            in_flight: Mutex::new(()),
            #[cfg(test)]
            stuck_read: Mutex::new(None),
        }
    }

    /// Whether a timed-out read is still running.
    pub(crate) fn is_stuck(&self) -> bool {
        self.stuck.load(Ordering::SeqCst) != 0
    }

    /// Runs `read` on its own thread and waits at most `timeout` on `clock`.
    ///
    /// Reads are serialized (see `in_flight`), so a caller may wait for an
    /// earlier read's deadline before its own read starts. The deadline is
    /// taken before the reader starts.
    ///
    /// On timeout the thread is left to finish (or stay blocked) on its own;
    /// it only reads the keystore and holds no state, so abandoning it is
    /// safe and the process can still exit. A thread that cannot be started,
    /// or that panics before answering, is [`DeadlineMiss::Failed`].
    pub(crate) fn run<T: Send + 'static>(
        &self,
        clock: &Clock,
        timeout: Duration,
        read: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T, DeadlineMiss> {
        // The guard holds no data, so a poisoned lock is still a valid one.
        let _one_at_a_time = self
            .in_flight
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if self.is_stuck() {
            return Err(DeadlineMiss::TimedOut);
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let now = clock.now();
        // A timeout past the end of `Instant` waits no time: the read is then a timeout.
        let deadline = now.checked_add(timeout).unwrap_or(now);
        let answer = Arc::new(Answer {
            value: Mutex::new(None),
            finished: CancelToken::new(),
        });
        let guard = Finished {
            id,
            finished: answer.finished.clone(),
            stuck: Arc::clone(&self.stuck),
        };
        let reader = Arc::clone(&answer);
        std::thread::Builder::new()
            .name("sc-keyring-read".to_owned())
            .spawn(move || {
                let _guard = guard;
                let value = read();
                *reader.value.lock().unwrap_or_else(PoisonError::into_inner) = Some(value);
            })
            .map_err(|_| DeadlineMiss::Failed)?;
        if clock.wait_until(&answer.finished, deadline) == Wait::Cancelled {
            return answer
                .value
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take()
                .ok_or(DeadlineMiss::Failed);
        }
        self.stuck.store(id, Ordering::SeqCst);
        // The reader may have finished between the deadline and the store
        // above; if so it will not clear the mark again.
        if answer.finished.is_cancelled() {
            let _ = self
                .stuck
                .compare_exchange(id, 0, Ordering::SeqCst, Ordering::SeqCst);
        }
        #[cfg(test)]
        {
            *self
                .stuck_read
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Some(answer.finished.clone());
        }
        Err(DeadlineMiss::TimedOut)
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
        match GATE.run(&Clock::system(), KEYRING_READ_TIMEOUT, move || {
            read_entry(&service, &account)
        }) {
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

    #[cfg(feature = "test-keyring-guard")]
    #[test]
    fn test_build_traps_real_platform_keyring_construction() {
        assert!(std::panic::catch_unwind(PlatformKeyring::new).is_err());
    }

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

    use std::sync::atomic::AtomicBool;

    use sc_agent_testkit::{Gate, HANG_GUARD, Started, cancelled_within, join_within};
    use sc_agent_types::testing::ManualClock;

    /// The read deadline of these tests; only an advance of the manual clock reaches it.
    const SHORT: Duration = Duration::from_millis(20);

    /// Waits (bounded) until the read that is stuck now has finished and cleared the mark.
    fn wait_until_unstuck(gate: &ReadGate) {
        let finished = gate
            .stuck_read
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .expect("a read is stuck");
        cancelled_within(&finished, HANG_GUARD);
        assert!(!gate.is_stuck(), "the finished reader cleared the mark");
    }

    /// A read that never answers returns a typed error at the deadline instead of hanging.
    /// The deadline is reached by advancing a manual clock once the read is known to be in
    /// flight; the blocked reader waits at a gate the test opens.
    #[test]
    fn a_blocked_keyring_read_times_out_with_a_typed_error() {
        let manual = ManualClock::new();
        let gate = Arc::new(ReadGate::new());
        let (started, release) = (Started::new(), Gate::new());
        let opener = release.opener();
        let caller = {
            let (gate, clock) = (Arc::clone(&gate), manual.clock());
            let (started, release) = (started.clone(), release.clone());
            std::thread::spawn(move || {
                gate.run(&clock, SHORT, move || {
                    started.signal();
                    release.wait_open();
                    1_u8
                })
            })
        };
        started.wait_for(1);
        assert!(manual.wait_for_waiters(1), "the caller waits on the clock");
        manual.advance(SHORT);
        assert_eq!(
            join_within(caller, HANG_GUARD).expect("caller"),
            Err(DeadlineMiss::TimedOut)
        );
        assert!(gate.is_stuck());
        // While it is stuck a later read fails at once and never runs.
        let ran = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&ran);
        let later = gate.run(&manual.clock(), SHORT, move || {
            flag.store(true, Ordering::SeqCst);
        });
        assert_eq!(later, Err(DeadlineMiss::TimedOut));
        assert!(!ran.load(Ordering::SeqCst));
        // Releasing the stuck reader clears the mark, and reads work again.
        opener.open();
        wait_until_unstuck(&gate);
        assert_eq!(gate.run(&manual.clock(), SHORT, || 7_u8), Ok(7));
    }

    /// Two callers whose reads would both block start one reader thread between them: both
    /// get `TimedOut` and at most one thread is stranded. The first read is known to be in
    /// flight before the clock reaches its deadline, so the second caller, whichever thread it
    /// is, finds the stuck mark.
    #[test]
    fn two_concurrent_blocked_reads_strand_one_thread() {
        let manual = ManualClock::new();
        let gate = Arc::new(ReadGate::new());
        let (started, release) = (Started::new(), Gate::new());
        let opener = release.opener();
        let callers: Vec<_> = (0..2)
            .map(|index| {
                let (gate, clock) = (Arc::clone(&gate), manual.clock());
                let (started, release) = (started.clone(), release.clone());
                std::thread::Builder::new()
                    .name(format!("caller-{index}"))
                    .spawn(move || {
                        gate.run(&clock, SHORT, move || {
                            started.signal();
                            release.wait_open();
                        })
                    })
                    .expect("spawn a caller")
            })
            .collect();
        started.wait_for(1);
        assert!(
            manual.wait_for_waiters(1),
            "the first caller waits on the clock"
        );
        manual.advance(SHORT);
        for caller in callers {
            assert_eq!(
                join_within(caller, HANG_GUARD).expect("caller"),
                Err(DeadlineMiss::TimedOut)
            );
        }
        assert_eq!(started.count(), 1, "one reader thread only");
        assert!(gate.is_stuck());
        opener.open();
        wait_until_unstuck(&gate);
        assert_eq!(gate.run(&manual.clock(), SHORT, || 9_u8), Ok(9));
    }

    /// A reader that panics before answering is not a timeout and leaves the gate usable;
    /// the clock never moves, so no deadline is involved.
    #[test]
    fn a_panicking_keyring_read_is_not_a_timeout_and_the_next_read_works() {
        let clock = ManualClock::new().clock();
        let gate = ReadGate::new();
        let result = gate.run(&clock, SHORT, || -> u8 {
            panic!("keystore reader failed (expected in this test)")
        });
        assert_eq!(result, Err(DeadlineMiss::Failed));
        assert!(!gate.is_stuck());
        assert_eq!(gate.run(&clock, SHORT, || 3_u8), Ok(3));
        let error = keyring_read_failed();
        assert_eq!(error.id(), "secrets.keyring_read");
    }
}
