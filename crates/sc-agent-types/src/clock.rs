//! [`Clock`]: the one source of time and of timed waits in the workspace.
//!
//! Every production read of the current instant and every timed wait goes through a
//! [`Clock`], so a test can swap the real clock ([`SystemClock`]) for one that moves only when
//! the test moves it (`testing::ManualClock`, feature `testing`). Nothing here sleeps: a wait
//! parks on a condition variable and is woken by a cancel, by its deadline (real clock) or by
//! the test advancing the clock (manual clock).

use std::fmt;
use std::sync::{Arc, Condvar, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use crate::cancel::{CancelToken, Wait};
#[cfg(feature = "testing")]
use crate::testing::ManualClock;

/// A wake-up run when a clock's time moves by a call rather than by real time.
pub type Waker = Arc<dyn Fn() + Send + Sync>;

/// The real clock: [`Instant::now`], and waits that end when real time reaches the deadline.
///
/// Its waits park on a condition variable with a timeout to the deadline; they never poll and
/// never sleep a fixed delay.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SystemClock;

/// The source of time of a run: the real clock in production, a manual clock in tests.
///
/// Cheap to clone; clones share one source. [`Default`] is [`Clock::system`].
///
/// A component that owns its own `Mutex` + `Condvar` (a queue waiting for its earliest
/// holdoff) waits through [`Self::park`] and registers a [`Waker`] with
/// [`Self::on_advance`], so a manual clock can wake it; everything else waits through
/// [`Self::wait_until`].
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use sc_agent_types::{CancelToken, Clock, Wait};
///
/// let clock = Clock::system();
/// let token = CancelToken::new();
/// token.cancel();
/// let far = clock.now() + Duration::from_secs(60);
/// assert_eq!(clock.wait_until(&token, far), Wait::Cancelled);
/// ```
#[derive(Clone, Default)]
pub struct Clock(Source);

#[derive(Clone, Default)]
enum Source {
    #[default]
    System,
    #[cfg(feature = "testing")]
    Manual(ManualClock),
}

/// Keeps a [`Waker`] registered with [`Clock::on_advance`]; dropping it deregisters the waker.
///
/// For the real clock it holds nothing: real time needs no wake-up call.
#[must_use = "dropping an AdvanceHook deregisters its waker at once"]
pub struct AdvanceHook {
    #[cfg(feature = "testing")]
    _registration: Option<crate::testing::Registration>,
}

impl fmt::Debug for AdvanceHook {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdvanceHook")
            .finish_non_exhaustive()
    }
}

impl Clock {
    /// The real clock ([`SystemClock`]).
    #[must_use]
    pub fn system() -> Self {
        Self(Source::System)
    }

    /// The current instant of this clock.
    #[must_use]
    pub fn now(&self) -> Instant {
        match &self.0 {
            Source::System => Instant::now(),
            #[cfg(feature = "testing")]
            Source::Manual(manual) => manual.now(),
        }
    }

    /// Blocks until `cancel` is cancelled or this clock reaches `deadline`, whichever is first.
    ///
    /// Returns [`Wait::Cancelled`] at once if the token is already cancelled, and
    /// [`Wait::DeadlineReached`] at once if the deadline has passed. Parks on the token's
    /// condition variable; it does not poll.
    #[must_use]
    pub fn wait_until(&self, cancel: &CancelToken, deadline: Instant) -> Wait {
        cancel.park_until(self, deadline)
    }

    /// Parks once on `condvar` (paired with the mutex of `guard`): until a notify, or until this
    /// clock reaches `deadline`. The caller loops and re-checks its own state.
    ///
    /// On the real clock this is `Condvar::wait_timeout` to the deadline. On a manual clock it
    /// parks until notified; the caller registers a [`Waker`] with [`Self::on_advance`] that
    /// locks the same mutex and notifies `condvar`, so an advance wakes it. Returns at once when
    /// the deadline has already passed.
    pub fn park<'a, T>(
        &self,
        condvar: &Condvar,
        guard: MutexGuard<'a, T>,
        deadline: Instant,
    ) -> MutexGuard<'a, T> {
        match self.real_timeout(deadline) {
            None => guard,
            Some(Some(timeout)) => {
                condvar
                    .wait_timeout(guard, timeout)
                    .unwrap_or_else(PoisonError::into_inner)
                    .0
            }
            Some(None) => condvar.wait(guard).unwrap_or_else(PoisonError::into_inner),
        }
    }

    /// Registers `wake` to run each time this clock is moved by a call (a manual clock's
    /// advance). The real clock never calls it. Dropping the returned hook deregisters it.
    ///
    /// `wake` runs on the advancing thread, outside the clock's locks; it must not block.
    pub fn on_advance(&self, wake: Waker) -> AdvanceHook {
        match &self.0 {
            Source::System => {
                drop(wake);
                AdvanceHook {
                    #[cfg(feature = "testing")]
                    _registration: None,
                }
            }
            #[cfg(feature = "testing")]
            Source::Manual(manual) => AdvanceHook {
                _registration: Some(manual.register(wake)),
            },
        }
    }

    /// How a park for `deadline` waits: `None` when the deadline has passed, `Some(Some(t))`
    /// for a real-time wait of `t`, `Some(None)` for a wait that only a notify ends.
    fn real_timeout(&self, deadline: Instant) -> Option<Option<Duration>> {
        let now = self.now();
        if now >= deadline {
            return None;
        }
        match &self.0 {
            Source::System => Some(Some(deadline.saturating_duration_since(now))),
            #[cfg(feature = "testing")]
            Source::Manual(_) => Some(None),
        }
    }
}

impl From<SystemClock> for Clock {
    fn from(_: SystemClock) -> Self {
        Self::system()
    }
}

#[cfg(feature = "testing")]
impl From<ManualClock> for Clock {
    fn from(manual: ManualClock) -> Self {
        Self(Source::Manual(manual))
    }
}

impl fmt::Debug for Clock {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Source::System => formatter.write_str("Clock(system)"),
            #[cfg(feature = "testing")]
            Source::Manual(manual) => write!(formatter, "Clock({manual:?})"),
        }
    }
}
