//! Cooperative cancellation: [`CancelToken`], its waits and its wake hooks.
//!
//! Std-only and runtime-free: a holdoff waits on a condition variable with a deadline
//! ([`Clock::wait_until`](crate::Clock::wait_until)) and wakes at once on cancel; nothing
//! blocks on a fixed delay.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, Weak};
use std::thread::{self, ThreadId};
use std::time::Instant;

use crate::clock::Clock;

/// A hook run once on cancel.
type Hook = Arc<dyn Fn() + Send + Sync>;

/// A flag a caller sets to ask in-flight work to stop.
///
/// Clones share one flag. [`Default`] is a token that is never cancelled unless
/// [`Self::cancel`] is called on it or a clone. Two tokens are equal when they share the flag.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use sc_agent_types::{CancelToken, Clock, Wait};
///
/// let clock = Clock::system();
/// let token = CancelToken::new();
/// let worker = token.clone();
/// assert!(!worker.is_cancelled());
/// token.cancel();
/// assert!(worker.is_cancelled());
/// assert_eq!(clock.wait_until(&worker, clock.now() + Duration::from_secs(60)), Wait::Cancelled);
/// ```
#[derive(Clone, Default)]
pub struct CancelToken(Arc<Inner>);

// Why the fields are laid out this way:
// - `cancelled` is set while `state` is locked, and `park_until` checks it under the same lock
//   before parking on `wake`, so a waiter cannot check, miss the `notify_all` and then park.
// - `cancelled` is also an atomic so `is_cancelled` is lock-free: work loops poll it often.
// - The hook list, the running hook and both condition variables share the one `state` Mutex:
//   cancel, registration, drop and waiting all decide on one consistent view, and a condvar
//   must always be paired with the mutex that guards the state it signals.
// - A poisoned lock is recovered (`PoisonError::into_inner`), not propagated: the state is a
//   flag, a list and an `Option`, each left consistent at every unlock, so a panic elsewhere
//   cannot leave it half-written, and cancellation must keep working after it.
#[derive(Default)]
struct Inner {
    cancelled: AtomicBool,
    state: Mutex<Hooks>,
    /// Signalled on cancel, and by a manual clock's advance: wakes every `park_until`.
    wake: Condvar,
    /// Signalled each time a running hook finishes: wakes a `CancelHook::drop` waiting for it.
    hook_done: Condvar,
    /// Test seam (feature `testing`): told when cancel has taken a hook and when a drop
    /// starts waiting for a running hook, so a test forces each interleaving once.
    #[cfg(feature = "testing")]
    trace: Mutex<Option<Trace>>,
}

/// The `testing` trace callback.
#[cfg(feature = "testing")]
type Trace = Arc<dyn Fn(crate::testing::CancelEvent) + Send + Sync>;

#[derive(Default)]
struct Hooks {
    next: u64,
    /// Registered hooks that have not started. `cancel` takes them one at a time.
    pending: Vec<(u64, Hook)>,
    /// The hook `cancel` is running now, outside the lock, and the thread running it.
    running: Option<(u64, ThreadId)>,
}

/// How a [`Clock::wait_until`] ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wait {
    /// The token was cancelled (before or during the wait).
    Cancelled,
    /// The deadline passed with no cancel.
    DeadlineReached,
}

impl Inner {
    /// The hook list; a lock poisoned by a panicking holder is recovered, not propagated.
    fn lock(&self) -> MutexGuard<'_, Hooks> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Tells the `testing` trace, if one is set, that `event` happened.
    #[cfg(feature = "testing")]
    fn trace(&self, event: crate::testing::CancelEvent) {
        let trace = self
            .trace
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(trace) = trace {
            trace(event);
        }
    }
}

impl CancelToken {
    /// A fresh token that is not cancelled.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation. Idempotent.
    ///
    /// The first call wakes every [`Clock::wait_until`] and then runs every registered hook
    /// once, one at a time, on the calling thread. No hook runs while the token's lock is
    /// held. A later call returns at once, even while the first is still running hooks.
    pub fn cancel(&self) {
        {
            let _state = self.0.lock();
            if self.0.cancelled.swap(true, Ordering::AcqRel) {
                return;
            }
            self.0.wake.notify_all();
        }
        loop {
            // Take the next hook and mark it running in the same critical section, so a
            // concurrent `CancelHook::drop` either removes it first or sees it running.
            let hook = {
                let mut state = self.0.lock();
                let Some((id, hook)) = state.pending.pop() else {
                    return;
                };
                state.running = Some((id, thread::current().id()));
                hook
            };
            let _done = HookDone(&self.0);
            #[cfg(feature = "testing")]
            self.0.trace(crate::testing::CancelEvent::HookTaken);
            hook();
        }
    }

    /// Whether cancellation was requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }

    /// Blocks until the token is cancelled or `clock` reaches `deadline` (the body of
    /// [`Clock::wait_until`]).
    ///
    /// The waker registered with the clock locks `state` before it notifies, and the deadline
    /// is checked under that lock, so an advance cannot slip between the check and the park.
    pub(crate) fn park_until(&self, clock: &Clock, deadline: Instant) -> Wait {
        let mut state = self.0.lock();
        if self.is_cancelled() {
            return Wait::Cancelled;
        }
        let inner = Arc::downgrade(&self.0);
        let _advance = clock.on_advance(Arc::new(move || {
            if let Some(inner) = inner.upgrade() {
                let _state = inner.lock();
                inner.wake.notify_all();
            }
        }));
        loop {
            if self.is_cancelled() {
                return Wait::Cancelled;
            }
            if clock.now() >= deadline {
                return Wait::DeadlineReached;
            }
            state = clock.park(&self.0.wake, state, deadline);
        }
    }

    /// Sets the `testing` trace: `trace` is called with each [`CancelEvent`] of this token
    /// (and its clones). It runs on the thread of the event, `DropWaiting` with the token's
    /// lock held, so it must not call into this token; it may block (to hold that thread at
    /// the event) only for `HookTaken`.
    ///
    /// [`CancelEvent`]: crate::testing::CancelEvent
    #[cfg(feature = "testing")]
    pub fn set_trace(&self, trace: Arc<dyn Fn(crate::testing::CancelEvent) + Send + Sync>) {
        *self.0.trace.lock().unwrap_or_else(PoisonError::into_inner) = Some(trace);
    }

    /// Registers `wake` to run once on cancel, or at once if the token is already cancelled.
    ///
    /// `wake` must not block and must not panic: hooks run one after another on the thread
    /// that calls [`Self::cancel`] (or on this thread, if already cancelled), and a blocked
    /// hook holds up the others and any thread dropping its [`CancelHook`]. It runs outside
    /// the token's lock, so it may register hooks or drop any `CancelHook`, its own included.
    ///
    /// Dropping the returned `CancelHook` deregisters it. Once the drop returns, the hook has
    /// either finished or will never run (`CancelHook` is `#[must_use]`).
    pub fn on_cancel(&self, wake: Arc<dyn Fn() + Send + Sync>) -> CancelHook {
        let id = {
            let mut state = self.0.lock();
            if self.is_cancelled() {
                None
            } else {
                let id = state.next;
                state.next = state.next.wrapping_add(1);
                state.pending.push((id, wake.clone()));
                Some(id)
            }
        };
        match id {
            Some(id) => CancelHook {
                token: Arc::downgrade(&self.0),
                id,
            },
            None => {
                wake();
                CancelHook {
                    token: Weak::new(),
                    id: 0,
                }
            }
        }
    }
}

impl PartialEq for CancelToken {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for CancelToken {}

impl fmt::Debug for CancelToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CancelToken")
            .field("cancelled", &self.is_cancelled())
            .finish_non_exhaustive()
    }
}

/// A hook registered with [`CancelToken::on_cancel`]; dropping it deregisters the hook.
///
/// When the drop returns, the hook has finished or will never run. If `cancel` is running the
/// hook on another thread at that moment, the drop waits for it to finish. If the drop happens
/// on the thread running the hook (the hook drops its own `CancelHook`, directly or through
/// something it drops), it returns at once instead of waiting for itself: the hook is then
/// finishing on that same thread and does not run again.
#[derive(Debug)]
#[must_use = "dropping a CancelHook deregisters its hook at once"]
pub struct CancelHook {
    token: Weak<Inner>,
    id: u64,
}

impl Drop for CancelHook {
    fn drop(&mut self) {
        let Some(inner) = self.token.upgrade() else {
            return;
        };
        let mut state = inner.lock();
        if let Some(index) = state.pending.iter().position(|(id, _)| *id == self.id) {
            // Not started: removed under the lock, so `cancel` can no longer take it.
            drop(state.pending.swap_remove(index));
            return;
        }
        let me = thread::current().id();
        #[cfg(feature = "testing")]
        if state
            .running
            .is_some_and(|(id, thread)| id == self.id && thread != me)
        {
            inner.trace(crate::testing::CancelEvent::DropWaiting);
        }
        let _state = inner
            .hook_done
            .wait_while(state, |state| {
                state
                    .running
                    .is_some_and(|(id, thread)| id == self.id && thread != me)
            })
            .unwrap_or_else(PoisonError::into_inner);
    }
}

/// Clears `running` and wakes waiting drops when a hook returns, or unwinds on a panic, so a
/// drop never waits on a hook that has stopped.
struct HookDone<'a>(&'a Inner);

impl Drop for HookDone<'_> {
    fn drop(&mut self) {
        self.0.lock().running = None;
        self.0.hook_done.notify_all();
    }
}
