use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

use crate::Codec;
use sc_agent_types::Error;

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
    pub sends: u32,
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

/// The one place that sends: any [`Codec`], with retry, cancel and batching.
pub struct Client<C> {
    codec: C,
    agent: ureq::Agent,
    policy: Policy,
    /// Provider-wide pause: no sender on this client sends before this time.
    not_before: Mutex<Option<Instant>>,
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
        }
    }

    pub fn codec(&self) -> &C {
        &self.codec
    }

    /// Sends `request`; busy, network and invalid replies are resent until max sends. A busy
    /// reply pauses every sender on this client for its [`holdoff`] plus jitter; network and
    /// invalid replies wait the same holdoff (no Retry-After). Quota replies return at once.
    pub fn send(&self, request: &C::Request, cancel: &Cancel) -> Result<C::Response, Error> {
        self.send_counted(request, cancel).0
    }

    /// The same send path, retaining how many HTTP sends actually started.
    pub fn send_counted(
        &self,
        request: &C::Request,
        cancel: &Cancel,
    ) -> (Result<C::Response, Error>, u32) {
        let mut sends = 0;
        let mut last = Error::Cancelled { sends: 0 };
        for attempt in 1..=self.policy.max_sends {
            if self.wait_unpaused(cancel) {
                return (Err(Error::Cancelled { sends }), sends);
            }
            let wait = match self.once(request, attempt, &mut sends) {
                Ok(response) => return (Ok(response), sends),
                Err((error, _)) if !error.retryable() => return (Err(error), sends),
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
            if attempt < self.policy.max_sends && cancel.wait(wait) {
                return (Err(Error::Cancelled { sends }), sends);
            }
        }
        (Err(last), sends)
    }

    /// The time before which no sender on this client sends; `None` when never paused.
    pub fn not_before(&self) -> Option<Instant> {
        *self.not_before.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Waits until the client is not paused, re-reading the pause after each wait because
    /// another sender may have extended it. True when cancelled.
    fn wait_unpaused(&self, cancel: &Cancel) -> bool {
        loop {
            if cancel.is_cancelled() {
                return true;
            }
            let left = self.not_before().map_or(Duration::ZERO, |t| {
                t.saturating_duration_since(Instant::now())
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
        let until = Instant::now() + wait;
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
        sends: &mut u32,
    ) -> Result<C::Response, (Error, Option<Duration>)> {
        let encoded = self.codec.encode(request).map_err(|e| (e, None))?;
        *sends += 1;
        let mut reply = self
            .agent
            .run(encoded)
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
        let queue = Mutex::new(requests.into_iter().enumerate());
        let (tx, rx) = mpsc::channel();
        std::thread::scope(|scope| {
            for _ in 0..workers.max(1) {
                let (tx, queue) = (tx.clone(), &queue);
                scope.spawn(move || {
                    loop {
                        let next = queue.lock().unwrap_or_else(|e| e.into_inner()).next();
                        let Some((position, request)) = next else {
                            break;
                        };
                        let (result, sends) = self.send_counted(&request, cancel);
                        let item = Item {
                            position,
                            session: ulid::Ulid::new(),
                            request,
                            result,
                            sends,
                        };
                        if tx.send(item).is_err() {
                            break;
                        }
                    }
                });
            }
            drop(tx);
            rx.into_iter().for_each(&mut on_result);
        });
    }
}
