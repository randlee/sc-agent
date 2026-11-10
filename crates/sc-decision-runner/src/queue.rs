//! The two queues of a run: [`JobQueue`] (ready, held off and in-flight jobs) and
//! [`BoundedQueue`] (results waiting for the consumer).
//!
//! Both are a `Mutex` plus `Condvar`s. Nothing here sleeps: a held job is waited for
//! with `Condvar::wait_timeout` to the earliest holdoff end, and every state change that
//! a waiter may need (submit, hold, done, close, cancel, push, pop, interrupt) notifies.
//! No code runs while a lock is held except plain collection operations, so a poisoned
//! lock still guards a consistent state and is recovered.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

/// The run's clock: `Instant::now` in production, a test clock in unit tests.
pub(crate) type Clock = Arc<dyn Fn() -> Instant + Send + Sync>;

/// Locks `mutex`, recovering a poisoned lock (see the module docs).
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Why a job got its answer without being run to the end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Skipped {
    /// The queue was cancelled before the job finished.
    Cancelled,
    /// The run function panicked on this job; the payload text.
    Panicked(String),
}

/// What [`JobQueue::take`] hands a worker.
#[derive(Debug)]
pub(crate) enum Taken<J> {
    /// Run this job.
    Run(J),
    /// The queue is cancelled: answer this job as cancelled without running it.
    Cancelled(J),
    /// No job is left and none can come: the worker exits. `cancelled` is read under the
    /// queue lock with that decision: whether the queue was cancelled or drained.
    Finished {
        /// The queue was cancelled (else the input ended and drained).
        cancelled: bool,
    },
}

/// A job held off until `until`; `seq` keeps equal instants first in, first out.
struct Held<J> {
    until: Instant,
    seq: u64,
    job: J,
}

impl<J> Held<J> {
    fn key(&self) -> (Instant, u64) {
        (self.until, self.seq)
    }
}

impl<J> PartialEq for Held<J> {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl<J> Eq for Held<J> {}

impl<J> PartialOrd for Held<J> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<J> Ord for Held<J> {
    /// Reversed, so the max-heap `BinaryHeap` pops the earliest `until` first.
    fn cmp(&self, other: &Self) -> Ordering {
        other.key().cmp(&self.key())
    }
}

struct JobState<J, F> {
    ready: VecDeque<J>,
    held: BinaryHeap<Held<J>>,
    /// Jobs taken by a worker and not yet `done`.
    running: usize,
    /// Sends in progress, by key.
    in_flight: Vec<(u64, F)>,
    /// In-flight sends cut off by `cancel`, for the result iterator to answer.
    orphans: Vec<(u64, F)>,
    /// The input ended: no more submits.
    closed: bool,
    /// Cancelled: every waiting job is answered as cancelled, submits are refused.
    cancelled: bool,
    /// Next held sequence number and in-flight key.
    seq: u64,
    /// Submitters blocked at capacity (test seam, QA-R2-005; read under the lock).
    #[cfg(test)]
    waiting_submitters: usize,
}

/// The job queue of one run: ready FIFO, held min-heap, in-flight and orphan lists.
///
/// `J` is the job, `F` the in-flight entry kept while a job's send is in progress.
/// Capacity counts ready + held + running jobs, so held-off jobs also throttle the feeder.
pub(crate) struct JobQueue<J, F> {
    state: Mutex<JobState<J, F>>,
    changed: Condvar,
    capacity: usize,
    clock: Clock,
}

impl<J, F> JobQueue<J, F> {
    /// An empty queue of `capacity` jobs (at least one); held jobs fall due by `clock`.
    pub(crate) fn new(capacity: usize, clock: Clock) -> Self {
        Self {
            state: Mutex::new(JobState {
                ready: VecDeque::new(),
                held: BinaryHeap::new(),
                running: 0,
                in_flight: Vec::new(),
                orphans: Vec::new(),
                closed: false,
                cancelled: false,
                seq: 0,
                #[cfg(test)]
                waiting_submitters: 0,
            }),
            changed: Condvar::new(),
            capacity: capacity.max(1),
            clock,
        }
    }

    fn lock(&self) -> MutexGuard<'_, JobState<J, F>> {
        lock(&self.state)
    }

    /// Queues `job`, blocking while the queue is at capacity.
    ///
    /// # Errors
    ///
    /// The job back once the queue is closed or cancelled (a blocked submit is woken).
    pub(crate) fn submit(&self, job: J) -> Result<(), J> {
        let mut state = self.lock();
        loop {
            if state.closed || state.cancelled {
                return Err(job);
            }
            if state.ready.len() + state.held.len() + state.running < self.capacity {
                state.ready.push_back(job);
                drop(state);
                self.changed.notify_all();
                return Ok(());
            }
            #[cfg(test)]
            {
                state.waiting_submitters += 1;
                self.changed.notify_all();
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
            #[cfg(test)]
            {
                state.waiting_submitters -= 1;
            }
        }
    }

    /// Holds `job` until `until`. Never blocks. Call before [`Self::done`] for that job.
    pub(crate) fn hold(&self, job: J, until: Instant) {
        {
            let mut state = self.lock();
            let seq = state.seq;
            state.seq = seq.wrapping_add(1);
            state.held.push(Held { until, seq, job });
        }
        self.changed.notify_all();
    }

    /// The next job for a worker: due held jobs move to the back of the ready queue,
    /// then the oldest ready job; else waits to the earliest holdoff end, or for a change.
    pub(crate) fn take(&self) -> Taken<J> {
        let mut state = self.lock();
        loop {
            if state.cancelled {
                let job = state
                    .ready
                    .pop_front()
                    .or_else(|| state.held.pop().map(|held| held.job));
                return match job {
                    Some(job) => {
                        state.running += 1;
                        Taken::Cancelled(job)
                    }
                    // A running job that is held later is taken as cancelled by its own worker.
                    None => Taken::Finished { cancelled: true },
                };
            }
            let now = (self.clock)();
            while state.held.peek().is_some_and(|held| held.until <= now) {
                if let Some(held) = state.held.pop() {
                    state.ready.push_back(held.job);
                }
            }
            if let Some(job) = state.ready.pop_front() {
                state.running += 1;
                return Taken::Run(job);
            }
            if let Some(earliest) = state.held.peek().map(|held| held.until) {
                let wait = earliest.saturating_duration_since(now);
                state = self
                    .changed
                    .wait_timeout(state, wait)
                    .unwrap_or_else(PoisonError::into_inner)
                    .0;
                continue;
            }
            if state.closed && state.running == 0 {
                return Taken::Finished { cancelled: false };
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// A taken job is finished, requeued or held.
    pub(crate) fn done(&self) {
        {
            let mut state = self.lock();
            state.running = state.running.saturating_sub(1);
        }
        self.changed.notify_all();
    }

    /// The input ended; workers finish once the queue drains.
    pub(crate) fn close(&self) {
        self.lock().closed = true;
        self.changed.notify_all();
    }

    /// Cancels the queue: [`Self::take`] yields every ready and held job as
    /// [`Taken::Cancelled`], submits are refused, every in-flight entry becomes an
    /// orphan. Never blocks beyond the lock; safe in a `CancelToken` hook.
    pub(crate) fn cancel(&self) {
        {
            let mut state = self.lock();
            state.cancelled = true;
            let in_flight = std::mem::take(&mut state.in_flight);
            state.orphans.extend(in_flight);
        }
        self.changed.notify_all();
    }

    /// Whether [`Self::cancel`] was called.
    pub(crate) fn is_cancelled(&self) -> bool {
        self.lock().cancelled
    }

    /// Registers a send about to start and returns its key.
    ///
    /// # Errors
    ///
    /// The entry back when the queue is already cancelled: the job must not be sent.
    pub(crate) fn start_send(&self, entry: F) -> Result<u64, F> {
        let mut state = self.lock();
        if state.cancelled {
            return Err(entry);
        }
        let key = state.seq;
        state.seq = key.wrapping_add(1);
        state.in_flight.push((key, entry));
        Ok(key)
    }

    /// The send of `key` returned: removes its entry from the in-flight or orphan list.
    pub(crate) fn end_send(&self, key: u64) {
        let removed = {
            let mut guard = self.lock();
            let state = &mut *guard;
            let mut removed = Vec::new();
            for list in [&mut state.in_flight, &mut state.orphans] {
                if let Some(index) = list.iter().position(|(entry, _)| *entry == key) {
                    removed.push(list.swap_remove(index));
                }
            }
            removed
        };
        // Entries are dropped outside the lock.
        drop(removed);
    }

    /// Once cancelled, every ready and held job, for the result iterator to answer at once
    /// (workers may all be blocked in sends); empty before a cancel.
    pub(crate) fn take_cancelled(&self) -> Vec<J> {
        let jobs: Vec<J> = {
            let mut state = self.lock();
            if !state.cancelled {
                return Vec::new();
            }
            let held = std::mem::take(&mut state.held);
            let mut jobs: Vec<J> = state.ready.drain(..).collect();
            jobs.extend(held.into_iter().map(|held| held.job));
            jobs
        };
        self.changed.notify_all();
        jobs
    }

    /// Every in-flight entry cut off by [`Self::cancel`] and not yet taken.
    pub(crate) fn take_orphans(&self) -> Vec<F> {
        let orphans = std::mem::take(&mut self.lock().orphans);
        orphans.into_iter().map(|(_, entry)| entry).collect()
    }
}

struct QueueState<T> {
    items: VecDeque<T>,
    /// Producers still alive; the queue closes when the last one is done.
    producers: usize,
    /// No more pushes. Pops drain what is left, then return `Finished`.
    closed: bool,
    /// Nobody will pop: pushes are refused and queued items are dropped.
    discarded: bool,
    /// Bumped by `interrupt`: a waiting pop returns `Interrupted`.
    generation: u64,
    /// Pushers blocked on a full queue (test seam, QA-R2-005; read under the lock).
    #[cfg(test)]
    waiting_pushers: usize,
}

/// What [`BoundedQueue::pop`] found.
#[derive(Debug)]
pub(crate) enum Popped<T> {
    /// The next item.
    Item(T),
    /// [`BoundedQueue::interrupt`] was called since the caller read the generation.
    Interrupted,
    /// Closed (or discarded) and drained.
    Finished,
}

/// A bounded multi-producer FIFO of results, closed when its last producer is done.
pub(crate) struct BoundedQueue<T> {
    state: Mutex<QueueState<T>>,
    not_empty: Condvar,
    not_full: Condvar,
    capacity: usize,
}

impl<T> BoundedQueue<T> {
    /// An empty queue of `capacity` items (at least one) fed by `producers` producers.
    pub(crate) fn new(capacity: usize, producers: usize) -> Self {
        Self {
            state: Mutex::new(QueueState {
                items: VecDeque::new(),
                producers,
                closed: producers == 0,
                discarded: false,
                generation: 0,
                #[cfg(test)]
                waiting_pushers: 0,
            }),
            not_empty: Condvar::new(),
            not_full: Condvar::new(),
            capacity: capacity.max(1),
        }
    }

    fn lock(&self) -> MutexGuard<'_, QueueState<T>> {
        lock(&self.state)
    }

    /// Pushes `item`, blocking while the queue is full.
    ///
    /// # Errors
    ///
    /// The item back when the queue is closed or discarded (the consumer is gone).
    pub(crate) fn push(&self, item: T) -> Result<(), T> {
        let mut state = self.lock();
        loop {
            if state.closed || state.discarded {
                return Err(item);
            }
            if state.items.len() < self.capacity {
                state.items.push_back(item);
                drop(state);
                self.not_empty.notify_all();
                return Ok(());
            }
            #[cfg(test)]
            {
                state.waiting_pushers += 1;
                self.not_empty.notify_all();
            }
            state = self
                .not_full
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
            #[cfg(test)]
            {
                state.waiting_pushers -= 1;
            }
        }
    }

    /// The current interrupt generation; pass it to [`Self::pop`].
    pub(crate) fn generation(&self) -> u64 {
        self.lock().generation
    }

    /// Pops the next item, blocking while empty; returns `Interrupted` once the
    /// generation differs from `seen`.
    pub(crate) fn pop(&self, seen: u64) -> Popped<T> {
        let mut state = self.lock();
        loop {
            if let Some(item) = state.items.pop_front() {
                drop(state);
                self.not_full.notify_one();
                return Popped::Item(item);
            }
            if state.closed || state.discarded {
                return Popped::Finished;
            }
            if state.generation != seen {
                return Popped::Interrupted;
            }
            state = self
                .not_empty
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Wakes every waiting pop without closing the queue. Never blocks beyond the lock.
    pub(crate) fn interrupt(&self) {
        {
            let mut state = self.lock();
            state.generation = state.generation.wrapping_add(1);
        }
        self.not_empty.notify_all();
    }

    /// One producer is done; the last one closes the queue.
    pub(crate) fn producer_done(&self) {
        {
            let mut state = self.lock();
            state.producers = state.producers.saturating_sub(1);
            if state.producers == 0 {
                state.closed = true;
            }
        }
        self.not_empty.notify_all();
        self.not_full.notify_all();
    }

    /// The consumer is gone: drops queued items and refuses pushes.
    pub(crate) fn discard(&self) {
        let dropped = {
            let mut state = self.lock();
            state.discarded = true;
            std::mem::take(&mut state.items)
        };
        // Items are dropped outside the lock.
        drop(dropped);
        self.not_empty.notify_all();
        self.not_full.notify_all();
    }

    /// Whether [`Self::discard`] was called.
    pub(crate) fn is_discarded(&self) -> bool {
        self.lock().discarded
    }
}

#[cfg(test)]
impl<J, F> JobQueue<J, F> {
    /// Waits (bounded) until `count` submitters are blocked at capacity; their count then.
    pub(crate) fn wait_for_waiting_submitters(
        &self,
        count: usize,
        failsafe: std::time::Duration,
    ) -> usize {
        let (state, _) = self
            .changed
            .wait_timeout_while(self.lock(), failsafe, |state| {
                state.waiting_submitters < count
            })
            .unwrap_or_else(PoisonError::into_inner);
        state.waiting_submitters
    }
}

#[cfg(test)]
impl<T> BoundedQueue<T> {
    /// Waits (bounded) until `count` pushers are blocked on a full queue; their count then.
    pub(crate) fn wait_for_waiting_pushers(
        &self,
        count: usize,
        failsafe: std::time::Duration,
    ) -> usize {
        let (state, _) = self
            .not_empty
            .wait_timeout_while(self.lock(), failsafe, |state| state.waiting_pushers < count)
            .unwrap_or_else(PoisonError::into_inner);
        state.waiting_pushers
    }
}

#[cfg(test)]
mod tests;
