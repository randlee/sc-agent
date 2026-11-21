//! Test doubles of the time seam (feature `testing`, not default).
//!
//! Consumers take `sc-agent-types = { path = .., features = ["testing"] }` as a
//! dev-dependency only. [`ManualClock`] is a [`Clock`] that moves only when the test calls
//! [`ManualClock::advance`]: a wait on it ends when the test advances it past the deadline (or
//! on cancel), never by real time. [`CancelEvent`] names the points of a [`CancelToken`] a
//! test can stop at ([`CancelToken::set_trace`]).
//!
//! # Examples
//!
//! ```
//! use std::thread;
//! use std::time::Duration;
//!
//! use sc_agent_types::testing::ManualClock;
//! use sc_agent_types::{CancelToken, Clock, Wait};
//!
//! let manual = ManualClock::new();
//! let clock = Clock::from(manual.clone());
//! let deadline = clock.now() + Duration::from_secs(30);
//! let waiter = thread::spawn(move || clock.wait_until(&CancelToken::new(), deadline));
//! assert!(manual.wait_for_waiters(1), "the waiter parked");
//! manual.advance(Duration::from_secs(30));
//! assert_eq!(waiter.join().ok(), Some(Wait::DeadlineReached));
//! ```
//!
//! [`CancelToken`]: crate::CancelToken
//! [`CancelToken::set_trace`]: crate::CancelToken::set_trace

use std::fmt;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, Weak};
use std::time::{Duration, Instant};

use crate::clock::{Clock, Waker};

/// Locks `mutex`, recovering a poisoned lock: every guarded value is changed in single steps.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A point inside a [`CancelToken`](crate::CancelToken) reported to its test trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CancelEvent {
    /// `cancel` has taken a hook and marked it running; the hook has not been called yet.
    HookTaken,
    /// A `CancelHook` drop found its hook running on another thread and is about to wait.
    DropWaiting,
}

/// A clock that moves only when the test moves it. Clones share one instant.
///
/// Waits on it ([`Clock::wait_until`], [`Clock::park`]) register a waker for as long as they
/// are parked; [`Self::advance`] wakes them all, and [`Self::wait_for_waiters`] tells a test
/// that a thread is parked, so the test advances only once it is.
#[derive(Clone)]
pub struct ManualClock {
    inner: Arc<Manual>,
}

struct Manual {
    now: Mutex<Instant>,
    wakers: Mutex<Wakers>,
    /// Signalled when a waker is registered: wakes [`ManualClock::wait_for_waiters`].
    registered: Condvar,
}

#[derive(Default)]
struct Wakers {
    next: u64,
    list: Vec<(u64, Waker)>,
}

/// A waker's registration on a [`ManualClock`]; dropping it removes the waker.
pub(crate) struct Registration {
    clock: Weak<Manual>,
    id: u64,
}

impl Drop for Registration {
    fn drop(&mut self) {
        if let Some(clock) = self.clock.upgrade() {
            let removed = {
                let mut wakers = lock(&clock.wakers);
                let index = wakers.list.iter().position(|(id, _)| *id == self.id);
                index.map(|index| wakers.list.swap_remove(index))
            };
            // The waker is dropped outside the lock.
            drop(removed);
        }
    }
}

impl ManualClock {
    /// The longest [`Self::wait_for_waiters`] waits before it gives up: a guard, never a
    /// measurement.
    pub const FAILSAFE: Duration = Duration::from_secs(60);

    /// A clock reading an arbitrary instant (the moment it was made); only differences between
    /// its readings mean anything.
    #[must_use]
    pub fn new() -> Self {
        Self::starting_at(Instant::now())
    }

    /// A clock reading `start`.
    #[must_use]
    pub fn starting_at(start: Instant) -> Self {
        Self {
            inner: Arc::new(Manual {
                now: Mutex::new(start),
                wakers: Mutex::default(),
                registered: Condvar::new(),
            }),
        }
    }

    /// The current reading.
    #[must_use]
    pub fn now(&self) -> Instant {
        *lock(&self.inner.now)
    }

    /// Moves the clock forward by `by` and wakes every parked wait, which then re-reads it.
    pub fn advance(&self, by: Duration) {
        {
            let mut now = lock(&self.inner.now);
            *now += by;
        }
        // Called outside every lock of this clock: a waker locks its waiter's own mutex.
        let wakers: Vec<Waker> = lock(&self.inner.wakers)
            .list
            .iter()
            .map(|(_, wake)| Arc::clone(wake))
            .collect();
        for wake in wakers {
            wake();
        }
    }

    /// Moves the clock forward to `at`; an `at` at or before the current reading changes
    /// nothing.
    pub fn advance_to(&self, at: Instant) {
        let by = at.saturating_duration_since(self.now());
        self.advance(by);
    }

    /// Waits until at least `waiters` waits are parked on this clock; `true` when they
    /// are. `false` after [`Self::FAILSAFE`], so a broken ordering fails the test instead of
    /// hanging it. Callers assert the result.
    #[must_use]
    pub fn wait_for_waiters(&self, waiters: usize) -> bool {
        let wakers = lock(&self.inner.wakers);
        let (_wakers, timeout) = self
            .inner
            .registered
            .wait_timeout_while(wakers, Self::FAILSAFE, |wakers| wakers.list.len() < waiters)
            .unwrap_or_else(PoisonError::into_inner);
        !timeout.timed_out()
    }

    /// Waits parked on this clock now.
    #[must_use]
    pub fn waiters(&self) -> usize {
        lock(&self.inner.wakers).list.len()
    }

    /// This clock as a [`Clock`].
    #[must_use]
    pub fn clock(&self) -> Clock {
        Clock::from(self.clone())
    }

    /// Registers `wake` until the returned registration is dropped.
    pub(crate) fn register(&self, wake: Waker) -> Registration {
        let id = {
            let mut wakers = lock(&self.inner.wakers);
            let id = wakers.next;
            wakers.next = wakers.next.wrapping_add(1);
            wakers.list.push((id, wake));
            id
        };
        self.inner.registered.notify_all();
        Registration {
            clock: Arc::downgrade(&self.inner),
            id,
        }
    }
}

impl Default for ManualClock {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ManualClock {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManualClock")
            .field("now", &self.now())
            .field("waiters", &self.waiters())
            .finish()
    }
}
