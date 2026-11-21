//! Provider-wide pause and deadlines for generic queued work.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// Cancel flag; [`Cancel::wait`] ends at once when cancelled (no sleep).
#[derive(Clone, Default)]
pub struct Cancel(Arc<(Mutex<bool>, Condvar)>);

impl Cancel {
    pub fn cancel(&self) {
        let (flag, cvar) = &*self.0;
        *flag.lock().unwrap_or_else(|e| e.into_inner()) = true;
        cvar.notify_all();
    }

    pub fn is_cancelled(&self) -> bool {
        *self.0.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Waits up to `wait`; true when cancelled first.
    pub fn wait(&self, wait: Duration) -> bool {
        let (flag, cvar) = &*self.0;
        let deadline = deadline_after(Instant::now(), wait);
        let mut cancelled = flag.lock().unwrap_or_else(|e| e.into_inner());
        while !*cancelled {
            let Some(left) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            if left.is_zero() {
                return false;
            }
            cancelled = match cvar.wait_timeout(cancelled, left) {
                Ok((guard, _)) => guard,
                Err(e) => e.into_inner().0,
            };
        }
        true
    }
}

fn deadline_after(now: Instant, delay: Duration) -> Instant {
    if let Some(end) = now.checked_add(delay) {
        return end;
    }
    let (mut low, mut high) = (0_u64, delay.as_secs());
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if now.checked_add(Duration::from_secs(middle)).is_some() {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    now.checked_add(Duration::from_secs(low)).unwrap_or(now)
}

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

/// A queue of held work. Callers supply time so selection is deterministic.
pub struct Queue<T> {
    items: VecDeque<Held<T>>,
}

impl<T> Default for Queue<T> {
    fn default() -> Self {
        Self {
            items: VecDeque::new(),
        }
    }
}

impl<T> Queue<T> {
    /// Adds work that can be sent immediately.
    pub fn push(&mut self, item: T, now: Instant) {
        self.items.push_back(Held::new(item, now));
    }

    /// Requeues work until its holdoff ends.
    pub fn retry(&mut self, mut item: Held<T>, now: Instant, delay: Duration) {
        item.ready = deadline_after(now, delay);
        self.items.push_back(item);
    }

    /// Removes the first runnable, expired, or cancelled item, preserving held work.
    pub fn take(
        &mut self,
        now: Instant,
        cancelled: bool,
        pause: Option<Instant>,
    ) -> Option<Held<T>> {
        let index = self.items.iter().position(|item| {
            cancelled
                || item.expired(now)
                || (item.ready <= now && pause.is_none_or(|until| until <= now))
        })?;
        self.items.remove(index)
    }

    /// Duration until any item may become runnable or exhaust its budget.
    pub fn next_wait(&self, now: Instant, pause: Option<Instant>) -> Option<Duration> {
        self.items
            .iter()
            .map(|item| {
                let ready = pause.map_or(item.ready, |until| until.max(item.ready));
                let deadline = item
                    .started
                    .map_or(ready, |start| ready.min(start + BUDGET));
                deadline.saturating_duration_since(now)
            })
            .min()
    }
}

/// Uses Retry-After when supplied, otherwise doubles the configured backoff.
pub fn delay(backoff: Duration, sends: u32, retry_after: Option<Duration>) -> Duration {
    retry_after.unwrap_or_else(|| backoff.saturating_mul(1_u32 << sends.saturating_sub(1).min(31)))
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
pub struct ProviderPause {
    deadline: Mutex<Option<Instant>>,
    clock: Arc<dyn Fn() -> Instant + Send + Sync>,
}

impl Default for ProviderPause {
    fn default() -> Self {
        Self::with_clock(Instant::now)
    }
}

impl ProviderPause {
    /// Uses the supplied monotonic time source for scheduling and budget checks.
    pub fn with_clock(clock: impl Fn() -> Instant + Send + Sync + 'static) -> Self {
        Self {
            deadline: Mutex::new(None),
            clock: Arc::new(clock),
        }
    }

    fn now(&self) -> Instant {
        (self.clock)()
    }

    /// Returns the provider's current pause deadline.
    pub fn not_before(&self) -> Option<Instant> {
        *self.deadline.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn extend(&self, until: Instant) {
        let mut pause = self.deadline.lock().unwrap_or_else(|e| e.into_inner());
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
    limits: (usize, u32),
    pause: &ProviderPause,
    cancel: &crate::Cancel,
    attempt: impl Fn(&T, u32) -> Attempt<R, E> + Sync,
    cancelled: impl Fn(u32) -> E + Sync,
    mut result: impl FnMut(usize, T, Result<R, E>),
) {
    use std::sync::{Mutex, mpsc};
    let (workers, max_sends) = limits;
    let now = pause.now();
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
                    let now = pause.now();
                    let deadline = pause.not_before();
                    let Some(mut held) = state.queue.take(now, *guard, deadline) else {
                        if state.running == 0 && state.queue.items.is_empty() {
                            break;
                        }
                        let wait = state.queue.next_wait(now, deadline);
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
                    let expired = held.expired(now);
                    let outcome = if was_cancelled {
                        Attempt::Done(Err(cancelled(held.sends)))
                    } else if let Some(error) = held.item.last.take_if(|_| expired) {
                        Attempt::Done(Err(error))
                    } else {
                        held.sent(now);
                        attempt(&held.item.item, held.sends)
                    };
                    let _guard = flag.lock().unwrap_or_else(|e| e.into_inner());
                    let mut state = batch.lock().unwrap_or_else(|e| e.into_inner());
                    state.running -= 1;
                    if let Attempt::Retry {
                        delay, busy: true, ..
                    } = &outcome
                    {
                        let now = pause.now();
                        pause.extend(deadline_after(now, *delay));
                    }
                    match outcome {
                        Attempt::Retry { error, delay, .. } if held.sends < max_sends.max(1) => {
                            held.item.last = Some(error);
                            let now = pause.now();
                            state.queue.retry(held, now, delay);
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
    let mut held = Held::new(item, pause.now());
    let mut last = None;
    loop {
        if cancel.is_cancelled() {
            return Err(cancelled(held.sends));
        }
        let now = pause.now();
        if held.expired(now)
            && let Some(error) = last.take()
        {
            return Err(error);
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
                let now = pause.now();
                held.ready = deadline_after(now, delay);
                if busy {
                    pause.extend(held.ready);
                }
                if held.sends >= max_sends.max(1) {
                    return Err(error);
                }
                last = Some(error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn huge_retry_after_keeps_a_far_future_provider_pause() {
        let now = Instant::now();
        let end = deadline_after(now, Duration::MAX);
        assert!(end.saturating_duration_since(now) > BUDGET);
        let pause = ProviderPause::default();
        pause.extend(end);
        assert_eq!(pause.not_before(), Some(end));
        let mut queue = Queue::default();
        queue.push(1, now);
        let item = queue.take(now, false, None);
        assert!(item.is_some());
        if let Some(mut item) = item {
            item.sent(now);
            queue.retry(item, now, Duration::MAX);
        }
        assert_eq!(queue.next_wait(now, pause.not_before()), Some(BUDGET));
        assert!(pause.not_before().is_some_and(|value| value > now + BUDGET));
    }

    #[test]
    fn held_item_yields_to_ready_work_and_expires() {
        let now = Instant::now();
        let mut queue = Queue::default();
        queue.push(1, now);
        let mut first = queue.take(now, false, None).unwrap();
        first.sent(now);
        queue.retry(first, now, Duration::from_secs(100));
        queue.push(2, now);
        assert_eq!(queue.take(now, false, None).unwrap().item, 2);
        assert_eq!(queue.next_wait(now, None), Some(BUDGET));
        assert!(
            queue
                .take(now + BUDGET, false, None)
                .unwrap()
                .expired(now + BUDGET)
        );
    }

    #[test]
    fn budget_does_not_cut_a_send_in_flight() {
        use std::sync::atomic::{AtomicU64, Ordering};
        let start = Instant::now();
        let elapsed = Arc::new(AtomicU64::new(0));
        let clock = elapsed.clone();
        let pause = ProviderPause::with_clock(move || {
            start + Duration::from_secs(clock.load(Ordering::SeqCst))
        });
        let cancel = Cancel::default();
        let result = run_one(
            &(),
            5,
            &pause,
            &cancel,
            |_, _| {
                elapsed.store(91, Ordering::SeqCst);
                Attempt::<_, &str>::Done(Ok("finished"))
            },
            |_| "cancelled",
        );
        assert_eq!(result, Ok("finished"));
    }

    #[test]
    fn cancellation_stops_a_queued_retry_without_another_send() {
        let cancel = crate::Cancel::default();
        let pause = ProviderPause::default();
        let mut calls = 0;
        let result = run_one(
            &(),
            5,
            &pause,
            &cancel,
            |_, _| {
                calls += 1;
                cancel.cancel();
                Attempt::<(), _>::Retry {
                    error: "busy",
                    delay: Duration::from_secs(3),
                    busy: true,
                }
            },
            |_| "cancelled",
        );
        assert_eq!(calls, 1);
        assert_eq!(result, Err("cancelled"));
    }

    #[test]
    fn batch_retries_and_returns_each_original_item() {
        let cancel = crate::Cancel::default();
        let pause = ProviderPause::default();
        let mut received = Vec::new();
        run(
            vec![7, 8],
            (1, 2),
            &pause,
            &cancel,
            |item, sends| {
                if sends == 1 {
                    Attempt::Retry {
                        error: "busy",
                        delay: Duration::ZERO,
                        busy: false,
                    }
                } else {
                    Attempt::Done(Ok(*item * 2))
                }
            },
            |_| "cancelled",
            |position, item, result| received.push((position, item, result)),
        );
        received.sort_by_key(|item| item.0);
        assert_eq!(received, [(0, 7, Ok(14)), (1, 8, Ok(16))]);
    }

    #[test]
    fn shared_pause_and_cancel_are_driven_by_supplied_time() {
        let now = Instant::now();
        let mut queue = Queue::default();
        queue.push(1, now);
        let first = queue.take(now, false, None).unwrap();
        queue.retry(first, now, Duration::from_secs(3));
        queue.push(2, now);
        assert!(
            queue
                .take(now, false, Some(now + Duration::from_secs(3)))
                .is_none()
        );
        assert_eq!(
            queue.next_wait(now, Some(now + Duration::from_secs(3))),
            Some(Duration::from_secs(3))
        );
        assert!(
            queue
                .take(now, true, Some(now + Duration::from_secs(3)))
                .is_some()
        );
        assert!(
            queue
                .take(
                    now + Duration::from_secs(3),
                    false,
                    Some(now + Duration::from_secs(3))
                )
                .is_some()
        );
    }
}
