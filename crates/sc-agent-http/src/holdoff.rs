//! Provider-wide pause and deadlines for generic queued work.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// The maximum time between the first send and a later resend.
pub const BUDGET: Duration = Duration::from_secs(90);

/// One queued item and its retry timing, independent of the item itself.
pub struct Held<T> {
    /// The work to resume.
    pub item: T,
    /// Number of sends already made.
    pub sends: u32,
    started: Option<Instant>,
    ready: Instant,
}

impl<T> Held<T> {
    /// Creates an item ready for its first send.
    pub fn new(item: T, now: Instant) -> Self {
        Self {
            item,
            sends: 0,
            started: None,
            ready: now,
        }
    }

    /// Records a send and starts its budget on the first send.
    pub fn sent(&mut self, now: Instant) {
        self.started.get_or_insert(now);
        self.sends += 1;
    }

    /// Returns whether the budget has expired.
    pub fn expired(&self, now: Instant) -> bool {
        self.started
            .is_some_and(|start| now.saturating_duration_since(start) >= BUDGET)
    }
}

/// A queue and shared provider pause. Callers supply time so selection is deterministic.
pub struct Queue<T> {
    items: VecDeque<Held<T>>,
    pause: Option<Instant>,
}

impl<T> Default for Queue<T> {
    fn default() -> Self {
        Self {
            items: VecDeque::new(),
            pause: None,
        }
    }
}

impl<T> Queue<T> {
    /// Adds work that can be sent immediately.
    pub fn push(&mut self, item: T, now: Instant) {
        self.items.push_back(Held::new(item, now));
    }

    /// Requeues work until its holdoff ends. A busy reply also pauses the provider.
    pub fn retry(&mut self, mut item: Held<T>, now: Instant, delay: Duration, busy: bool) {
        item.ready = now.checked_add(delay).unwrap_or(now + BUDGET);
        if busy && self.pause.is_none_or(|pause| pause < item.ready) {
            self.pause = Some(item.ready);
        }
        self.items.push_back(item);
    }

    /// Removes the first runnable, expired, or cancelled item, preserving held work.
    pub fn take(&mut self, now: Instant, cancelled: bool) -> Option<Held<T>> {
        let index = self.items.iter().position(|item| {
            cancelled
                || item.expired(now)
                || (item.ready <= now && self.pause.is_none_or(|pause| pause <= now))
        })?;
        self.items.remove(index)
    }

    /// The time before which the provider is paused.
    pub fn not_before(&self) -> Option<Instant> {
        self.pause
    }

    /// Duration until any item may become runnable or exhaust its budget.
    pub fn next_wait(&self, now: Instant) -> Option<Duration> {
        self.items
            .iter()
            .map(|item| {
                let ready = self.pause.map_or(item.ready, |pause| pause.max(item.ready));
                let deadline = item
                    .started
                    .map_or(ready, |start| ready.min(start + BUDGET));
                deadline.saturating_duration_since(now)
            })
            .min()
    }
}

/// A retry decision returned by the caller after one send.
pub enum Attempt<R, E> {
    /// The request completed or failed without a retry.
    Done(Result<R, E>),
    /// Retry after this delay; a busy provider pauses all calls sharing the pause.
    Retry {
        error: E,
        delay: Duration,
        busy: bool,
    },
}

/// A pause shared by all calls to the same provider.
#[derive(Default)]
pub struct ProviderPause(std::sync::Mutex<Option<Instant>>);

impl ProviderPause {
    /// Returns the provider's current pause deadline.
    pub fn not_before(&self) -> Option<Instant> {
        *self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn extend(&self, until: Instant) {
        let mut pause = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if pause.is_none_or(|previous| previous < until) {
            *pause = Some(until);
        }
    }
}

struct Work<T, E> {
    position: usize,
    item: T,
    last: Option<E>,
}

struct Batch<T, E> {
    queue: Queue<Work<T, E>>,
    running: usize,
}

/// Runs queued work on a bounded worker pool. The operation controls which errors retry.
/// A held item remains in the queue, leaving workers free for other runnable work.
pub fn run<T: Send, R: Send, E: Send>(
    items: Vec<T>,
    workers: usize,
    max_sends: u32,
    pause: &ProviderPause,
    cancel: &crate::Cancel,
    attempt: impl Fn(&T, u32) -> Attempt<R, E> + Sync,
    cancelled: impl Fn(u32) -> E + Sync,
    mut result: impl FnMut(usize, T, Result<R, E>),
) {
    use std::sync::{Mutex, mpsc};
    let now = Instant::now();
    let mut queue = Queue::default();
    for (position, item) in items.into_iter().enumerate() {
        queue.push(
            Work {
                position,
                item,
                last: None,
            },
            now,
        );
    }
    let batch = Mutex::new(Batch { queue, running: 0 });
    let (tx, rx) = mpsc::channel();
    std::thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            let tx = tx.clone();
            let (batch, attempt, cancelled) = (&batch, &attempt, &cancelled);
            scope.spawn(move || {
                loop {
                    let (flag, wake) = &*cancel.0;
                    let mut guard = flag.lock().unwrap_or_else(|e| e.into_inner());
                    let mut state = batch.lock().unwrap_or_else(|e| e.into_inner());
                    let now = Instant::now();
                    if let Some(deadline) = pause.not_before() {
                        state.queue.pause =
                            Some(state.queue.pause.map_or(deadline, |p| p.max(deadline)));
                    }
                    let Some(mut held) = state.queue.take(now, *guard) else {
                        if state.running == 0 && state.queue.items.is_empty() {
                            break;
                        }
                        let wait = state.queue.next_wait(now);
                        drop(state);
                        guard = match wait {
                            Some(delay) => {
                                wake.wait_timeout(guard, delay)
                                    .unwrap_or_else(|e| e.into_inner())
                                    .0
                            }
                            None => wake.wait(guard).unwrap_or_else(|e| e.into_inner()),
                        };
                        drop(guard);
                        continue;
                    };
                    state.running += 1;
                    drop(state);
                    let was_cancelled = *guard;
                    drop(guard);
                    let outcome = if was_cancelled {
                        Attempt::Done(Err(cancelled(held.sends)))
                    } else if held.expired(now) {
                        Attempt::Done(Err(held.item.last.take().expect("expired work was sent")))
                    } else {
                        held.sent(now);
                        attempt(&held.item.item, held.sends)
                    };
                    let _guard = flag.lock().unwrap_or_else(|e| e.into_inner());
                    let mut state = batch.lock().unwrap_or_else(|e| e.into_inner());
                    state.running -= 1;
                    match outcome {
                        Attempt::Retry { error, delay, busy } if held.sends < max_sends.max(1) => {
                            held.item.last = Some(error);
                            let now = Instant::now();
                            if busy {
                                pause.extend(now.checked_add(delay).unwrap_or(now + BUDGET));
                            }
                            state.queue.retry(held, now, delay, busy);
                        }
                        outcome => {
                            let answer = match outcome {
                                Attempt::Done(answer) => answer,
                                Attempt::Retry { error, .. } => Err(error),
                            };
                            let work = held.item;
                            let _ = tx.send((work.position, work.item, answer));
                        }
                    }
                    wake.notify_all();
                }
            });
        }
        drop(tx);
        for (position, item, answer) in rx {
            result(position, item, answer);
        }
    });
}

/// Runs one borrowed item without imposing thread-safety bounds on it.
pub fn run_one<T, R, E>(
    item: &T,
    max_sends: u32,
    pause: &ProviderPause,
    cancel: &crate::Cancel,
    mut attempt: impl FnMut(&T, u32) -> Attempt<R, E>,
    cancelled: impl Fn(u32) -> E,
) -> Result<R, E> {
    let mut held = Held::new(item, Instant::now());
    let mut last = None;
    loop {
        if cancel.is_cancelled() {
            return Err(cancelled(held.sends));
        }
        let now = Instant::now();
        if held.expired(now) {
            return Err(last.expect("expired work was sent"));
        }
        let ready = pause.not_before().map_or(held.ready, |p| p.max(held.ready));
        if ready > now {
            let budget = held
                .started
                .map_or(ready, |start| ready.min(start + BUDGET));
            if cancel.wait(budget.saturating_duration_since(now)) {
                return Err(cancelled(held.sends));
            }
            continue;
        }
        held.sent(now);
        match attempt(item, held.sends) {
            Attempt::Done(result) => return result,
            Attempt::Retry { error, delay, busy } => {
                if held.sends >= max_sends.max(1) {
                    return Err(error);
                }
                last = Some(error);
                let now = Instant::now();
                held.ready = now.checked_add(delay).unwrap_or(now + BUDGET);
                if busy {
                    pause.extend(held.ready);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_item_yields_to_ready_work_and_expires() {
        let now = Instant::now();
        let mut queue = Queue::default();
        queue.push(1, now);
        let mut first = queue.take(now, false).unwrap();
        first.sent(now);
        queue.retry(first, now, Duration::from_secs(100), false);
        queue.push(2, now);
        assert_eq!(queue.take(now, false).unwrap().item, 2);
        assert_eq!(queue.next_wait(now), Some(BUDGET));
        assert!(
            queue
                .take(now + BUDGET, false)
                .unwrap()
                .expired(now + BUDGET)
        );
    }

    #[test]
    fn shared_pause_and_cancel_are_driven_by_supplied_time() {
        let now = Instant::now();
        let mut queue = Queue::default();
        queue.push(1, now);
        let first = queue.take(now, false).unwrap();
        queue.retry(first, now, Duration::from_secs(3), true);
        queue.push(2, now);
        assert!(queue.take(now, false).is_none());
        assert_eq!(queue.next_wait(now), Some(Duration::from_secs(3)));
        assert!(queue.take(now, true).is_some());
        assert!(queue.take(now + Duration::from_secs(3), false).is_some());
    }
}
