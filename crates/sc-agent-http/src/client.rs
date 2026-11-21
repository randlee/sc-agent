use std::collections::VecDeque;
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
        let mut last_retry_after_seconds = None;
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
                Err((mut error @ Error::ProviderBusy { .. }, retry_after)) => {
                    if let Error::ProviderBusy {
                        retry_after_seconds,
                        ..
                    } = &mut error
                    {
                        last_retry_after_seconds =
                            (*retry_after_seconds).or(last_retry_after_seconds);
                        *retry_after_seconds = last_retry_after_seconds;
                    }
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
        let now = self.now();
        let queue = Mutex::new(
            requests
                .into_iter()
                .enumerate()
                .map(|(position, request)| {
                    (
                        position,
                        request,
                        ulid::Ulid::new(),
                        0_u32,
                        None::<Instant>,
                        now,
                        None::<Error>,
                    )
                })
                .collect::<VecDeque<_>>(),
        );
        let (tx, rx) = mpsc::channel();
        std::thread::scope(|scope| {
            for _ in 0..workers.max(1) {
                let (tx, queue) = (tx.clone(), &queue);
                scope.spawn(move || {
                    loop {
                        let (flag, wake) = &*cancel.0;
                        let mut cancelled = flag.lock().unwrap_or_else(|e| e.into_inner());
                        let mut job = loop {
                            let mut q = queue.lock().unwrap_or_else(|e| e.into_inner());
                            if q.is_empty() {
                                return;
                            }
                            let now = self.now();
                            let pause = self.not_before().unwrap_or(now);
                            if let Some(i) = q.iter().position(|j| {
                                *cancelled
                                    || j.4.is_some_and(|start| {
                                        now.saturating_duration_since(start) >= BUDGET
                                    })
                                    || (j.5 <= now && pause <= now)
                            }) && let Some(job) = q.remove(i)
                            {
                                break job;
                            }
                            let wait = q
                                .iter()
                                .map(|j| {
                                    j.4.map_or(j.5.max(pause), |start| {
                                        j.5.max(pause).min(start + BUDGET)
                                    })
                                    .saturating_duration_since(now)
                                })
                                .min()
                                .unwrap_or_default();
                            drop(q);
                            cancelled = wake
                                .wait_timeout(cancelled, wait)
                                .unwrap_or_else(|e| e.into_inner())
                                .0;
                        };
                        let stopped = *cancelled;
                        drop(cancelled);
                        let result = if stopped {
                            Some(Err(Error::Cancelled { sends: job.3 }))
                        } else if job.4.is_some_and(|start| {
                            self.now().saturating_duration_since(start) >= BUDGET
                        }) {
                            let mut error =
                                job.6.take().unwrap_or(Error::Cancelled { sends: job.3 });
                            if let Error::ProviderBusy { reason, .. } = &mut error {
                                *reason = BusyReason::Budget;
                            }
                            Some(Err(error))
                        } else {
                            job.4.get_or_insert_with(|| self.now());
                            job.3 += 1;
                            match self.once(&job.1, job.3) {
                                Ok(value) => Some(Ok(value)),
                                Err((error, _)) if !error.retryable() => Some(Err(error)),
                                Err((error, retry_after)) => {
                                    let busy = matches!(error, Error::ProviderBusy { .. });
                                    let wait =
                                        holdoff(self.policy.backoff, job.3, retry_after) + jitter();
                                    if busy {
                                        self.pause(wait);
                                    }
                                    if job.3 >= self.policy.max_sends {
                                        Some(Err(error))
                                    } else {
                                        job.5 = self.now() + wait;
                                        job.6 = Some(error);
                                        None
                                    }
                                }
                            }
                        };
                        let _guard = flag.lock().unwrap_or_else(|e| e.into_inner());
                        if let Some(result) = result {
                            if tx
                                .send(Item {
                                    position: job.0,
                                    request: job.1,
                                    session: job.2,
                                    result,
                                })
                                .is_err()
                            {
                                return;
                            }
                        } else {
                            queue
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .push_back(job);
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
        network: bool,
    }

    impl Codec for AdvanceOnSend {
        type Request = ();
        type Response = ();
        fn encode(&self, _: &()) -> Result<crate::http::Request<Vec<u8>>, Error> {
            self.seconds.store(90, Ordering::SeqCst);
            if self.network {
                return Err(Error::Network("timeout".into()));
            }
            crate::post_json(&self.base, None, &[], &serde_json::json!({}))
        }
        fn decode(&self, _: &(), _: u16, _: &[u8]) -> Result<(), Error> {
            Err(Error::Invalid("unexpected response".into()))
        }
    }

    struct YieldOnRetry(String, Cancel);

    impl Codec for YieldOnRetry {
        type Request = u8;
        type Response = u8;
        fn encode(&self, request: &u8) -> Result<crate::http::Request<Vec<u8>>, Error> {
            if *request == 2 {
                self.1.cancel();
            }
            crate::post_json(&self.0, None, &[], &serde_json::json!({"n": request}))
        }
        fn decode(&self, request: &u8, _: u16, _: &[u8]) -> Result<u8, Error> {
            if *request == 1 {
                Err(Error::Invalid("retry".into()))
            } else {
                Ok(*request)
            }
        }
    }

    #[test]
    fn held_retry_yields_to_ready_request() {
        let server = crate::test_server::Server::start(vec![
            (200, vec![], "{}".into()),
            (200, vec![], "{}".into()),
        ]);
        let cancel = Cancel::default();
        let mut client = Client::new(
            YieldOnRetry(server.base.clone(), cancel.clone()),
            Policy::default(),
        );
        let start = Instant::now();
        client.clock = Some(Arc::new(move || start));
        let mut results = Vec::new();
        client.batch(vec![1, 2], 1, &cancel, |item| {
            results.push((item.request, item.result))
        });
        assert!(matches!(
            results.as_slice(),
            [(2, Ok(2)), (1, Err(Error::Cancelled { sends: 1 }))]
        ));
        assert_eq!(server.finish().len(), 2);
    }

    #[test]
    fn busy_request_exhausts_its_budget_without_another_send() {
        for (batch, network) in [(false, false), (true, false), (true, true)] {
            let replies = if network {
                vec![]
            } else {
                vec![(503, vec![("retry-after", "3".into())], "{}".into())]
            };
            let server = crate::test_server::Server::start(replies);
            let seconds = Arc::new(AtomicU64::new(0));
            let mut client = Client::new(
                AdvanceOnSend {
                    base: server.base.clone(),
                    seconds: seconds.clone(),
                    network,
                },
                Policy::default(),
            );
            let start = Instant::now();
            client.clock = Some(Arc::new(move || {
                start + Duration::from_secs(seconds.load(Ordering::SeqCst))
            }));
            let result = if batch {
                let mut result = None;
                client.batch(vec![()], 1, &Cancel::default(), |item| {
                    result = Some(item.result)
                });
                result.expect("batch emits one result")
            } else {
                client.send(&(), &Cancel::default())
            };
            if network {
                assert!(matches!(result, Err(Error::Network(message)) if message == "timeout"));
            } else {
                assert!(matches!(
                    result,
                    Err(Error::ProviderBusy {
                        attempts: 1,
                        reason: BusyReason::Budget,
                        retry_after_seconds: Some(3)
                    })
                ));
            }
            assert_eq!(server.finish().len(), usize::from(!network));
        }
    }
}
