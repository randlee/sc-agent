use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

use crate::Codec;
use sc_agent_types::{BusyReason, Error};

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
        let deadline = Instant::now() + wait;
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

/// How hard to try.
#[derive(Clone, Copy, Debug)]
pub struct Policy {
    pub max_sends: u32,
    pub backoff: Duration,
    pub timeout: Duration,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            max_sends: 5,
            backoff: Duration::from_secs(1),
            timeout: Duration::from_secs(120),
        }
    }
}

/// One batch result, matched to its input.
#[derive(Debug)]
pub struct Item<Q, R> {
    pub position: usize,
    pub session: ulid::Ulid,
    pub request: Q,
    pub result: Result<R, Error>,
}

const QUOTA: [&str; 4] = [
    "insufficient_quota",
    "credit_balance_exhausted",
    "billing_error",
    "enforced_spend_limit_reached",
];
const MAX_BODY: u64 = 64 * 1024 * 1024;
const BUDGET: Duration = Duration::from_secs(90);

struct Job<Q> {
    position: usize,
    request: Q,
    session: ulid::Ulid,
    attempts: u32,
    started: Option<Instant>,
    ready: Instant,
    last: Option<Error>,
}

impl<Q> Job<Q> {
    fn expired(&self, now: Instant) -> bool {
        self.started
            .is_some_and(|start| now.duration_since(start) >= BUDGET)
    }

    fn exhausted(&mut self, budget: bool) -> Error {
        let mut error = self.last.take().unwrap_or(Error::Cancelled {
            sends: self.attempts,
        });
        if let Error::ProviderBusy { reason, .. } = &mut error {
            *reason = if budget {
                BusyReason::Budget
            } else {
                BusyReason::Sends
            };
        }
        error
    }
}

fn ready<Q>(jobs: &[Job<Q>], now: Instant, pause: Instant, cancelled: bool) -> Option<usize> {
    jobs.iter()
        .position(|job| cancelled || job.expired(now) || (job.ready <= now && pause <= now))
}

/// The one place that sends: any [`Codec`], with retry, cancel and batching.
pub struct Client<C> {
    codec: C,
    agent: ureq::Agent,
    policy: Policy,
    /// Provider-wide pause: no sender on this client sends before this time.
    not_before: Mutex<Option<Instant>>,
    #[cfg(test)]
    clock: Option<Arc<dyn Fn() -> Instant + Send + Sync>>,
}

/// A valid Retry-After is clamped to this range (REQ-SAG-0601).
pub const RETRY_AFTER_MIN: Duration = Duration::from_millis(250);
/// The longest holdoff before jitter, from Retry-After or from backoff.
pub const HOLDOFF_MAX: Duration = Duration::from_secs(30);
/// Every holdoff adds up to this much jitter.
pub const JITTER_MAX: Duration = Duration::from_millis(250);

/// A valid Retry-After: 1 to 32 ASCII characters, digits with an optional fractional part.
/// Parsed exactly, unclamped: whole seconds past `u64::MAX` saturate to `u64::MAX`, and a
/// fraction finer than a nanosecond rounds up to the next nanosecond.
pub fn retry_after(text: &str) -> Option<Duration> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, "0"));
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if text.len() > 32 || !digits(whole) || !digits(fraction) {
        return None;
    }
    let nanos = format!("{fraction:0<9}")[..9].parse().ok()?;
    let finer = fraction.bytes().skip(9).any(|b| b != b'0');
    let exact = Duration::new(whole.parse().unwrap_or(u64::MAX), nanos);
    Some(exact.saturating_add(Duration::from_nanos(u64::from(finer))))
}

/// The holdoff before jitter: a valid Retry-After clamped to 0.25..=30 s, else `backoff`
/// doubled per attempt (1, 2, 4, 8 s with the default policy) and capped at 30 s.
pub fn holdoff(backoff: Duration, attempt: u32, retry_after: Option<Duration>) -> Duration {
    match retry_after {
        Some(wait) => wait.clamp(RETRY_AFTER_MIN, HOLDOFF_MAX),
        None => backoff
            .saturating_mul(1 << attempt.saturating_sub(1).min(16))
            .min(HOLDOFF_MAX),
    }
}

fn jitter() -> Duration {
    JITTER_MAX.mul_f64((ulid::Ulid::new().random() % 1024) as f64 / 1024.0)
}

impl<C: Codec> Client<C> {
    pub fn new(codec: C, policy: Policy) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(policy.timeout))
            .max_redirects(0)
            .http_status_as_error(false)
            .build()
            .into();
        Self {
            codec,
            agent,
            policy: Policy {
                max_sends: policy.max_sends.max(1),
                ..policy
            },
            not_before: Mutex::new(None),
            #[cfg(test)]
            clock: None,
        }
    }

    fn now(&self) -> Instant {
        #[cfg(test)]
        if let Some(clock) = &self.clock {
            return clock();
        }
        Instant::now()
    }

    pub fn codec(&self) -> &C {
        &self.codec
    }

    /// Sends `request`; busy, network and invalid replies are resent until max sends. A busy
    /// reply pauses every sender on this client for its [`holdoff`] plus jitter; network and
    /// invalid replies wait the same holdoff (no Retry-After). Quota replies return at once.
    pub fn send(&self, request: &C::Request, cancel: &Cancel) -> Result<C::Response, Error> {
        let mut last = Error::Cancelled { sends: 0 };
        let mut started: Option<Instant> = None;
        for attempt in 1..=self.policy.max_sends {
            if self.wait_unpaused(cancel, started) {
                return Err(Error::Cancelled { sends: attempt - 1 });
            }
            if started.is_some_and(|start| self.now().saturating_duration_since(start) >= BUDGET) {
                if let Error::ProviderBusy { reason, .. } = &mut last {
                    *reason = BusyReason::Budget;
                }
                return Err(last);
            }
            started.get_or_insert_with(|| self.now());
            let wait = match self.once(request, attempt) {
                Ok(response) => return Ok(response),
                Err((error, _)) if !error.retryable() => return Err(error),
                Err((error @ Error::ProviderBusy { .. }, retry_after)) => {
                    last = error;
                    self.pause(holdoff(self.policy.backoff, attempt, retry_after) + jitter());
                    continue;
                }
                Err((error, _)) => {
                    last = error;
                    holdoff(self.policy.backoff, attempt, None) + jitter()
                }
            };
            let remaining = started.map_or(wait, |start| {
                BUDGET
                    .saturating_sub(self.now().saturating_duration_since(start))
                    .min(wait)
            });
            if attempt < self.policy.max_sends && cancel.wait(remaining) {
                return Err(Error::Cancelled { sends: attempt });
            }
        }
        Err(last)
    }

    /// The time before which no sender on this client sends; `None` when never paused.
    pub fn not_before(&self) -> Option<Instant> {
        *self.not_before.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Waits until the client is not paused, re-reading the pause after each wait because
    /// another sender may have extended it. True when cancelled.
    fn wait_unpaused(&self, cancel: &Cancel, started: Option<Instant>) -> bool {
        loop {
            if cancel.is_cancelled() {
                return true;
            }
            let left = self
                .not_before()
                .map_or(Duration::ZERO, |t| t.saturating_duration_since(self.now()));
            let left = started.map_or(left, |start| {
                left.min(BUDGET.saturating_sub(self.now().saturating_duration_since(start)))
            });
            if left.is_zero() {
                return false;
            }
            if cancel.wait(left) {
                return true;
            }
        }
    }

    /// Extends the provider-wide pause to `wait` from now (at most 30.25 s, so no overflow).
    fn pause(&self, wait: Duration) {
        let until = self.now() + wait;
        let mut not_before = self.not_before.lock().unwrap_or_else(|e| e.into_inner());
        if not_before.is_none_or(|t| t < until) {
            *not_before = Some(until);
        }
    }

    /// One send. A retryable error comes back with the Retry-After wait, if any.
    fn once(
        &self,
        request: &C::Request,
        attempt: u32,
    ) -> Result<C::Response, (Error, Option<Duration>)> {
        let mut reply = self
            .agent
            .run(self.codec.encode(request).map_err(|e| (e, None))?)
            .map_err(|e| (Error::Network(e.to_string()), None))?;
        let status = reply.status().as_u16();
        let retry_after = reply
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(retry_after);
        let body = reply
            .body_mut()
            .with_config()
            .limit(MAX_BODY)
            .read_to_vec()
            .map_err(|e| (Error::Network(e.to_string()), None))?;
        if status != 429 && !(500..=599).contains(&status) {
            return self
                .codec
                .decode(request, status, &body)
                .map_err(|e| (e, None));
        }
        let text = String::from_utf8_lossy(&body);
        if QUOTA.iter().any(|marker| text.contains(marker)) {
            return Err((
                Error::QuotaExhausted {
                    status,
                    body: text.into_owned(),
                },
                None,
            ));
        }
        let busy = Error::ProviderBusy {
            attempts: attempt,
            reason: BusyReason::Sends,
            retry_after_seconds: retry_after
                .map(|d| d.as_secs().saturating_add(u64::from(d.subsec_nanos() > 0))),
        };
        Err((busy, retry_after))
    }

    /// Sends every request on `workers` threads. Results reach `on_result` on the caller's
    /// thread, in any order, each with its input position and a session ULID.
    pub fn batch(
        &self,
        requests: Vec<C::Request>,
        workers: usize,
        cancel: &Cancel,
        mut on_result: impl FnMut(Item<C::Request, C::Response>),
    ) where
        C::Request: Send,
        C::Response: Send,
    {
        // This mutex contains only the batch work queue; recovering poison retains queued work.
        let now = self.now();
        let count = requests.len();
        let queue = Mutex::new((
            requests
                .into_iter()
                .enumerate()
                .map(|(position, request)| Job {
                    position,
                    request,
                    session: ulid::Ulid::new(),
                    attempts: 0,
                    started: None,
                    ready: now,
                    last: None,
                })
                .collect::<Vec<_>>(),
            count,
        ));
        let (tx, rx) = mpsc::channel();
        std::thread::scope(|scope| {
            for _ in 0..workers.max(1) {
                let (tx, queue) = (tx.clone(), &queue);
                scope.spawn(move || {
                    loop {
                        let (flag, wake) = &*cancel.0;
                        let mut cancelled = flag.lock().unwrap_or_else(|e| e.into_inner());
                        let mut job = loop {
                            let mut state = queue.lock().unwrap_or_else(|e| e.into_inner());
                            if state.1 == 0 {
                                return;
                            }
                            let now = self.now();
                            let pause = self.not_before().unwrap_or(now);
                            let next = ready(&state.0, now, pause, *cancelled);
                            if let Some(i) = next {
                                break state.0.swap_remove(i);
                            }
                            let wait = state
                                .0
                                .iter()
                                .map(|j| {
                                    let ready = j.ready.max(pause);
                                    let deadline =
                                        j.started.map_or(ready, |start| ready.min(start + BUDGET));
                                    deadline.saturating_duration_since(now)
                                })
                                .min();
                            drop(state);
                            cancelled = match wait {
                                Some(wait) => {
                                    wake.wait_timeout(cancelled, wait)
                                        .unwrap_or_else(|e| e.into_inner())
                                        .0
                                }
                                None => wake.wait(cancelled).unwrap_or_else(|e| e.into_inner()),
                            };
                        };
                        let was_cancelled = *cancelled;
                        drop(cancelled);
                        let result = if was_cancelled {
                            Some(Err(Error::Cancelled {
                                sends: job.attempts,
                            }))
                        } else if job.expired(self.now()) {
                            Some(Err(job.exhausted(true)))
                        } else {
                            job.started.get_or_insert_with(|| self.now());
                            job.attempts += 1;
                            match self.once(&job.request, job.attempts) {
                                Ok(response) => Some(Ok(response)),
                                Err((error, _)) if !error.retryable() => Some(Err(error)),
                                Err((error, retry_after)) => {
                                    let busy = matches!(error, Error::ProviderBusy { .. });
                                    job.last = Some(error);
                                    let wait =
                                        holdoff(self.policy.backoff, job.attempts, retry_after)
                                            + jitter();
                                    if busy {
                                        self.pause(wait);
                                    }
                                    if job.attempts >= self.policy.max_sends {
                                        Some(Err(job.exhausted(false)))
                                    } else {
                                        job.ready = self.now() + wait;
                                        None
                                    }
                                }
                            }
                        };
                        let _guard = flag.lock().unwrap_or_else(|e| e.into_inner());
                        let mut state = queue.lock().unwrap_or_else(|e| e.into_inner());
                        if state.1 == 0 {
                            return;
                        }
                        if let Some(result) = result {
                            state.1 -= 1;
                            if tx
                                .send(Item {
                                    position: job.position,
                                    session: job.session,
                                    request: job.request,
                                    result,
                                })
                                .is_err()
                            {
                                state.1 = 0;
                            }
                        } else {
                            state.0.push(job);
                        }
                        wake.notify_all();
                    }
                });
            }
            drop(tx);
            rx.into_iter().for_each(&mut on_result);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct AdvanceOnSend {
        base: String,
        seconds: Arc<AtomicU64>,
    }

    impl Codec for AdvanceOnSend {
        type Request = ();
        type Response = ();
        fn encode(&self, _: &()) -> Result<crate::http::Request<Vec<u8>>, Error> {
            self.seconds.store(90, Ordering::SeqCst);
            crate::post_json(&self.base, None, &[], &serde_json::json!({}))
        }
        fn decode(&self, _: &(), _: u16, _: &[u8]) -> Result<(), Error> {
            Err(Error::Invalid("unexpected response".into()))
        }
    }

    #[test]
    fn busy_request_exhausts_its_budget_without_another_send() {
        let server = crate::test_server::Server::start(vec![(503, vec![], "{}".into())]);
        let seconds = Arc::new(AtomicU64::new(0));
        let mut client = Client::new(
            AdvanceOnSend {
                base: server.base.clone(),
                seconds: seconds.clone(),
            },
            Policy::default(),
        );
        let start = Instant::now();
        client.clock = Some(Arc::new(move || {
            start + Duration::from_secs(seconds.load(Ordering::SeqCst))
        }));
        assert!(matches!(
            client.send(&(), &Cancel::default()),
            Err(Error::ProviderBusy {
                attempts: 1,
                reason: BusyReason::Budget,
                ..
            })
        ));
        assert_eq!(server.finish().len(), 1);
    }

    fn job(position: usize, started: Option<Instant>, ready: Instant) -> Job<usize> {
        Job {
            position,
            request: position,
            session: ulid::Ulid::nil(),
            attempts: 1,
            started,
            ready,
            last: Some(Error::ProviderBusy {
                attempts: 1,
                retry_after_seconds: Some(3),
                reason: BusyReason::Sends,
            }),
        }
    }

    #[test]
    fn queue_selects_ready_jobs_and_expires_or_cancels_without_waiting() {
        let start = Instant::now();
        let jobs = [
            job(0, Some(start), start + Duration::from_secs(10)),
            job(1, None, start),
        ];
        assert_eq!(ready(&jobs, start, start, false), Some(1));
        assert_eq!(ready(&jobs[..1], start, start, false), None);
        assert_eq!(ready(&jobs[..1], start, start, true), Some(0));
        assert_eq!(
            ready(&jobs[..1], start + BUDGET, start + BUDGET, false),
            Some(0)
        );
        assert!(!jobs[0].expired(start + BUDGET - Duration::from_nanos(1)));
        assert!(jobs[0].expired(start + BUDGET));
    }

    #[test]
    fn exhaustion_preserves_busy_details_and_distinguishes_budget() {
        let start = Instant::now();
        let mut by_budget = job(0, Some(start), start);
        assert!(matches!(
            by_budget.exhausted(true),
            Error::ProviderBusy {
                attempts: 1,
                retry_after_seconds: Some(3),
                reason: BusyReason::Budget
            }
        ));
        let mut by_sends = job(0, Some(start), start);
        assert!(matches!(
            by_sends.exhausted(false),
            Error::ProviderBusy {
                reason: BusyReason::Sends,
                ..
            }
        ));
        let mut network = job(0, Some(start), start);
        network.last = Some(Error::Network("timeout".into()));
        assert!(matches!(network.exhausted(true), Error::Network(_)));
    }
}
