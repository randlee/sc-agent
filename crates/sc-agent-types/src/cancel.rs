//! Cooperative cancellation: [`CancelToken`], its waits and its wake hooks.
//!
//! Std-only and runtime-free: a holdoff waits on a condition variable with a deadline and
//! wakes at once on cancel; nothing sleeps.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, Weak};
use std::time::Instant;

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
/// use std::time::{Duration, Instant};
/// use sc_agent_types::{CancelToken, Wait};
///
/// let token = CancelToken::new();
/// let worker = token.clone();
/// assert!(!worker.is_cancelled());
/// token.cancel();
/// assert!(worker.is_cancelled());
/// assert_eq!(worker.wait_until(Instant::now() + Duration::from_secs(60)), Wait::Cancelled);
/// ```
#[derive(Clone, Default)]
pub struct CancelToken(Arc<Inner>);

#[derive(Default)]
struct Inner {
    cancelled: AtomicBool,
    state: Mutex<Hooks>,
    wake: Condvar,
}

#[derive(Default)]
struct Hooks {
    next: u64,
    hooks: Vec<(u64, Hook)>,
}

/// How a [`CancelToken::wait_until`] ended.
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
}

impl CancelToken {
    /// A fresh token that is not cancelled.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation. Idempotent.
    ///
    /// The first call wakes every [`Self::wait_until`] and runs every registered hook once.
    /// No hook runs while the token's lock is held.
    pub fn cancel(&self) {
        let hooks = {
            let mut state = self.0.lock();
            if self.0.cancelled.swap(true, Ordering::AcqRel) {
                return;
            }
            let hooks = std::mem::take(&mut state.hooks);
            self.0.wake.notify_all();
            hooks
        };
        for (_, hook) in hooks {
            hook();
        }
    }

    /// Whether cancellation was requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }

    /// Blocks until the token is cancelled or `deadline` passes, whichever is first.
    ///
    /// Returns [`Wait::Cancelled`] at once if the token is already cancelled. This waits on a
    /// condition variable; it does not sleep or poll.
    #[must_use]
    pub fn wait_until(&self, deadline: Instant) -> Wait {
        let mut state = self.0.lock();
        loop {
            if self.is_cancelled() {
                return Wait::Cancelled;
            }
            let timeout = deadline.saturating_duration_since(Instant::now());
            if timeout.is_zero() {
                return Wait::DeadlineReached;
            }
            state = self
                .0
                .wake
                .wait_timeout_while(state, timeout, |_| !self.is_cancelled())
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /// Registers `wake` to run once on cancel, or at once if the token is already cancelled.
    ///
    /// `wake` must not block. It runs on the thread that calls [`Self::cancel`] (or this
    /// method), outside the token's lock, so it may register or drop other hooks. Dropping the
    /// returned [`CancelHook`] deregisters it; a dropped hook never runs (`CancelHook` is
    /// `#[must_use]`).
    pub fn on_cancel(&self, wake: Arc<dyn Fn() + Send + Sync>) -> CancelHook {
        let id = {
            let mut state = self.0.lock();
            if self.is_cancelled() {
                None
            } else {
                let id = state.next;
                state.next = state.next.wrapping_add(1);
                state.hooks.push((id, wake.clone()));
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
#[derive(Debug)]
#[must_use = "dropping a CancelHook deregisters its hook at once"]
pub struct CancelHook {
    token: Weak<Inner>,
    id: u64,
}

impl Drop for CancelHook {
    fn drop(&mut self) {
        if let Some(inner) = self.token.upgrade() {
            inner.lock().hooks.retain(|(id, _)| *id != self.id);
        }
    }
}
