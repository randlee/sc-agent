//! [`Gate`] and [`Started`]: one-shot latches whose waits are bounded by [`HANG_GUARD`].

use std::fmt;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use crate::wait::HANG_GUARD;

/// A counter and a condvar; every wait on it has a deadline.
#[derive(Default)]
struct Latch {
    count: Mutex<usize>,
    changed: Condvar,
}

impl Latch {
    fn lock(&self) -> MutexGuard<'_, usize> {
        self.count.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn add(&self) {
        *self.lock() += 1;
        self.changed.notify_all();
    }

    /// Whether the count reached `at_least` within `guard`.
    fn reached(&self, at_least: usize, guard: Duration) -> bool {
        let (_count, timeout) = self
            .changed
            .wait_timeout_while(self.lock(), guard, |count| *count < at_least)
            .unwrap_or_else(PoisonError::into_inner);
        !timeout.timed_out()
    }
}

/// A gate a thread waits at until the test opens it. Clones share one gate.
///
/// [`Self::wait_open`] is bounded by [`HANG_GUARD`]. Hold an [`Opener`] ([`Self::opener`]) on the
/// test side: it opens the gate when dropped, so a test that panics before it opens the gate
/// never strands the thread waiting at it.
///
/// # Examples
///
/// ```
/// use std::thread;
///
/// use sc_agent_testkit::{Gate, HANG_GUARD, join_within};
///
/// let gate = Gate::new();
/// let opener = gate.opener();
/// let waiting = gate.clone();
/// let worker = thread::spawn(move || waiting.wait_open());
/// opener.open();
/// assert!(join_within(worker, HANG_GUARD).is_ok());
/// ```
#[derive(Clone, Default)]
pub struct Gate {
    latch: Arc<Latch>,
}

impl Gate {
    /// A closed gate.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens the gate: every waiting and later [`Self::wait_open`] returns.
    pub fn open(&self) {
        self.latch.add();
    }

    /// Whether the gate is open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        *self.latch.lock() > 0
    }

    /// An [`Opener`] that opens this gate when it is dropped (or by [`Opener::open`]).
    pub fn opener(&self) -> Opener {
        Opener { gate: self.clone() }
    }

    /// Waits until the gate is open.
    ///
    /// # Panics
    ///
    /// When the gate is still closed after [`HANG_GUARD`].
    #[track_caller]
    pub fn wait_open(&self) {
        assert!(
            self.latch.reached(1, HANG_GUARD),
            "the gate was not opened within {HANG_GUARD:?}"
        );
    }
}

impl fmt::Debug for Gate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gate")
            .field("open", &self.is_open())
            .finish()
    }
}

/// Opens its [`Gate`] when dropped. Made by [`Gate::opener`].
#[derive(Debug)]
#[must_use = "dropping an Opener opens its gate at once"]
pub struct Opener {
    gate: Gate,
}

impl Opener {
    /// Opens the gate now.
    pub fn open(self) {
        drop(self);
    }
}

impl Drop for Opener {
    fn drop(&mut self) {
        self.gate.open();
    }
}

/// A signal from another thread that it has got inside (a send, a handler, a read).
/// Clones share one count.
///
/// # Examples
///
/// ```
/// use std::thread;
///
/// use sc_agent_testkit::Started;
///
/// let started = Started::new();
/// let inside = started.clone();
/// let _worker = thread::spawn(move || inside.signal());
/// started.wait_for(1);
/// assert_eq!(started.count(), 1);
/// ```
#[derive(Clone, Default)]
pub struct Started {
    latch: Arc<Latch>,
}

impl Started {
    /// Nothing has started.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Called by the other thread once it is inside.
    pub fn signal(&self) {
        self.latch.add();
    }

    /// How many signals have been given.
    #[must_use]
    pub fn count(&self) -> usize {
        *self.latch.lock()
    }

    /// Waits until at least `signals` signals have been given.
    ///
    /// # Panics
    ///
    /// When fewer came within [`HANG_GUARD`].
    #[track_caller]
    pub fn wait_for(&self, signals: usize) {
        assert!(
            self.latch.reached(signals, HANG_GUARD),
            "only {} of {signals} start signals within {HANG_GUARD:?}",
            self.count()
        );
    }
}

impl fmt::Debug for Started {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Started")
            .field("count", &self.count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dropped_opener_opens_the_gate() {
        let gate = Gate::new();
        assert!(!gate.is_open());
        drop(gate.opener());
        assert!(gate.is_open());
        gate.wait_open();
    }

    #[test]
    fn started_counts_its_signals() {
        let started = Started::new();
        started.signal();
        started.signal();
        started.wait_for(2);
        assert_eq!(started.count(), 2);
    }
}
