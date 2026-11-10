use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use sc_agent_types::CancelToken;

/// Why a job got its answer without being run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Skipped {
    /// The pool was cancelled before the job started.
    Cancelled,
    /// The run function panicked on this job.
    Panicked,
}

/// A bounded multi-producer multi-consumer FIFO.
struct BoundedQueue<T> {
    state: Mutex<QueueState<T>>,
    not_empty: Condvar,
    not_full: Condvar,
    capacity: usize,
}

struct QueueState<T> {
    items: VecDeque<T>,
    /// No more pushes. Pops drain what is left, then return `None`.
    closed: bool,
    /// Nobody will pop: pushes are refused and queued items are dropped.
    discarded: bool,
    /// Items ever pushed, counted under this lock with the push itself.
    pushed: usize,
    /// Items ever popped, counted under this lock with the pop itself.
    popped: usize,
}

/// Why a push did not enqueue; the item comes back.
pub(crate) enum PushError<T> {
    /// The queue is closed.
    Closed(T),
    /// The queue stayed full for the whole wait.
    Full(T),
}

/// What a timed pop found.
pub(crate) enum Popped<T> {
    /// The next item.
    Item(T),
    /// Nothing arrived in time; the queue is still open.
    Empty,
    /// Closed and drained.
    Finished,
}

impl<T> BoundedQueue<T> {
    fn new(capacity: usize) -> Self {
        Self {
            state: Mutex::new(QueueState {
                items: VecDeque::new(),
                closed: false,
                discarded: false,
                pushed: 0,
                popped: 0,
            }),
            not_empty: Condvar::new(),
            not_full: Condvar::new(),
            capacity,
        }
    }

    fn lock(&self) -> MutexGuard<'_, QueueState<T>> {
        // A poisoned queue is still a consistent queue: no code runs while
        // the lock is held except plain `VecDeque` operations.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Pushes `item`, waiting at most `wait` (forever when `None`) for room.
    fn push(&self, item: T, wait: Option<Duration>) -> Result<(), PushError<T>> {
        let deadline = wait.map(|wait| Instant::now() + wait);
        let mut state = self.lock();
        loop {
            if state.closed || state.discarded {
                return Err(PushError::Closed(item));
            }
            if state.items.len() < self.capacity {
                state.items.push_back(item);
                state.pushed += 1;
                drop(state);
                self.not_empty.notify_one();
                return Ok(());
            }
            state = match deadline {
                None => self
                    .not_full
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner),
                Some(deadline) => {
                    let left = deadline.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return Err(PushError::Full(item));
                    }
                    self.not_full
                        .wait_timeout(state, left)
                        .unwrap_or_else(PoisonError::into_inner)
                        .0
                }
            };
        }
    }

    /// Pops the next item, waiting at most `wait` (forever when `None`).
    fn pop(&self, wait: Option<Duration>) -> Popped<T> {
        let deadline = wait.map(|wait| Instant::now() + wait);
        let mut state = self.lock();
        loop {
            if let Some(item) = state.items.pop_front() {
                state.popped += 1;
                drop(state);
                self.not_full.notify_one();
                return Popped::Item(item);
            }
            if state.closed || state.discarded {
                return Popped::Finished;
            }
            state = match deadline {
                None => self
                    .not_empty
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner),
                Some(deadline) => {
                    let left = deadline.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return Popped::Empty;
                    }
                    self.not_empty
                        .wait_timeout(state, left)
                        .unwrap_or_else(PoisonError::into_inner)
                        .0
                }
            };
        }
    }

    fn pushed(&self) -> usize {
        self.lock().pushed
    }

    fn popped(&self) -> usize {
        self.lock().popped
    }

    fn close(&self) {
        self.lock().closed = true;
        self.not_empty.notify_all();
        self.not_full.notify_all();
    }

    fn discard(&self) {
        {
            let mut state = self.lock();
            state.discarded = true;
            state.items.clear();
        }
        self.not_empty.notify_all();
        self.not_full.notify_all();
    }
}
