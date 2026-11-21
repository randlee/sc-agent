use std::borrow::Borrow;
use std::collections::VecDeque;
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
        let guard = flag.lock().unwrap_or_else(|e| e.into_inner());
        *cvar
            .wait_timeout_while(guard, wait, |cancelled| !*cancelled)
            .unwrap_or_else(|e| e.into_inner())
            .0
    }
}

fn body_error(error: ureq::Error) -> Error {
    match error {
        ureq::Error::BodyExceedsLimit(limit) => Error::ResponseTooLarge { limit },
        other => Error::Network(other.to_string()),
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

/// The one place that sends: any [`Codec`], with retry, cancel and batching.
pub struct Client<C> {
    codec: C,
    agent: ureq::Agent,
    policy: Policy,
    /// Provider-wide pause: no sender on this client sends before this time.
    not_before: Mutex<Option<Instant>>,
}

/// Reads Retry-After seconds as a duration.
pub fn retry_after(text: &str) -> Option<Duration> {
    let seconds = text.parse::<f64>().ok()?;
    if seconds.is_nan() || seconds < 0.0 {
        return None;
    }
    Some(Duration::try_from_secs_f64(seconds).unwrap_or(Duration::MAX))
}

/// Returns the specified Retry-After or exponentially increasing backoff.
pub fn holdoff(backoff: Duration, attempt: u32, retry_after: Option<Duration>) -> Duration {
    retry_after
        .unwrap_or_else(|| (1..attempt.min(128)).fold(backoff, |wait, _| wait.saturating_mul(2)))
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

    /// Sends a request through the same queue used by batch calls.
    pub fn send(&self, request: &C::Request, cancel: &Cancel) -> Result<C::Response, Error> {
        let mut answer = None;
        self.dispatch(vec![request], 1, cancel, Instant::now, |_, _, result| {
            answer = Some(result)
        });
        answer.unwrap_or(Err(Error::Cancelled { sends: 0 }))
    }

    /// The time before which no sender on this client sends; `None` when never paused.
    pub fn not_before(&self) -> Option<Instant> {
        *self.not_before.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Extends the provider-wide pause to `wait` from now.
    fn pause(&self, wait: Duration, now: Instant) {
        let until = now
            .checked_add(wait)
            .unwrap_or_else(|| now + Duration::from_secs(3_153_600_000));
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
        encoded: ureq::http::Request<Vec<u8>>,
    ) -> Result<C::Response, (Error, Option<Duration>)> {
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
            .map_err(|e| (body_error(e), None))?;
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
        self.dispatch(
            requests,
            workers,
            cancel,
            Instant::now,
            |position, request, result| {
                on_result(Item {
                    position,
                    session: ulid::Ulid::new(),
                    request,
                    result,
                });
            },
        );
    }

    fn dispatch<Q: Borrow<C::Request> + Send>(
        &self,
        requests: Vec<Q>,
        workers: usize,
        cancel: &Cancel,
        now: impl Fn() -> Instant + Sync,
        mut on_result: impl FnMut(usize, Q, Result<C::Response, Error>),
    ) {
        let initial = now();
        let queue = Mutex::new((
            requests
                .into_iter()
                .enumerate()
                .map(|(i, q)| (i, q, 0_u32, initial, None::<Instant>, None::<Error>))
                .collect::<VecDeque<_>>(),
            0_usize,
        ));
        let (tx, rx) = mpsc::channel();
        std::thread::scope(|scope| {
            for _ in 0..workers.max(1) {
                let (queue, tx, clock) = (&queue, tx.clone(), &now);
                scope.spawn(move || {
                    loop {
                        let (flag, wake) = &*cancel.0;
                        let guard = flag.lock().unwrap_or_else(|e| e.into_inner());
                        let mut state = queue.lock().unwrap_or_else(|e| e.into_inner());
                        let now = clock();
                        let pause = self.not_before().unwrap_or(now);
                        let index = state.0.iter().position(|job| {
                            *guard
                                || job.4.is_some_and(|first| {
                                    now.saturating_duration_since(first) >= Duration::from_secs(90)
                                })
                                || (job.3 <= now && pause <= now)
                        });
                        let Some(index) = index else {
                            if state.0.is_empty() && state.1 == 0 {
                                break;
                            }
                            let wait = state
                                .0
                                .iter()
                                .map(|job| {
                                    let ready = job.3.max(pause);
                                    job.4
                                        .map_or(ready, |first| {
                                            ready.min(first + Duration::from_secs(90))
                                        })
                                        .saturating_duration_since(now)
                                })
                                .min();
                            drop(state);
                            if let Some(wait) = wait {
                                drop(
                                    wake.wait_timeout(guard, wait)
                                        .unwrap_or_else(|e| e.into_inner()),
                                );
                            } else {
                                drop(wake.wait(guard).unwrap_or_else(|e| e.into_inner()));
                            }
                            continue;
                        };
                        let Some((position, request, mut sends, _, mut first, mut last)) =
                            state.0.remove(index)
                        else {
                            continue;
                        };
                        state.1 += 1;
                        let mut cancelled = *guard;
                        drop(state);
                        drop(guard);
                        let mut expired = first.is_some_and(|start| {
                            now.saturating_duration_since(start) >= Duration::from_secs(90)
                        });
                        let mut encoding_failed = false;
                        let mut deferred = None;
                        let result = if cancelled {
                            Err((Error::Cancelled { sends }, None))
                        } else if expired && last.is_some() {
                            Err((last.take().unwrap_or(Error::Cancelled { sends }), None))
                        } else {
                            match self.codec.encode(request.borrow()) {
                                Err(error) => {
                                    encoding_failed = true;
                                    Err((error, None))
                                }
                                Ok(encoded) => {
                                    let before = clock();
                                    cancelled = cancel.is_cancelled();
                                    expired = first.is_some_and(|start| {
                                        before.saturating_duration_since(start)
                                            >= Duration::from_secs(90)
                                    });
                                    if cancelled {
                                        Err((Error::Cancelled { sends }, None))
                                    } else if expired && last.is_some() {
                                        Err((
                                            last.take().unwrap_or(Error::Cancelled { sends }),
                                            None,
                                        ))
                                    } else if let Some(until) =
                                        self.not_before().filter(|until| *until > before)
                                    {
                                        deferred = Some(until);
                                        Err((Error::Cancelled { sends }, None))
                                    } else {
                                        sends += 1;
                                        first.get_or_insert(before);
                                        self.once(request.borrow(), sends, encoded)
                                    }
                                }
                            }
                        };
                        let retry = if let Some(until) = deferred {
                            Ok((last, until))
                        } else {
                            match result {
                                Err((error, retry_after))
                                    if !cancelled
                                        && !expired
                                        && !encoding_failed
                                        && (error.retryable()
                                            || matches!(error, Error::ResponseParse(_))) =>
                                {
                                    let delay = holdoff(self.policy.backoff, sends, retry_after);
                                    let now = clock();
                                    if matches!(error, Error::ProviderBusy { .. }) {
                                        self.pause(delay, now);
                                    }
                                    if sends < self.policy.max_sends {
                                        Ok((
                                            Some(error),
                                            now.checked_add(delay).unwrap_or_else(|| {
                                                now + Duration::from_secs(3_153_600_000)
                                            }),
                                        ))
                                    } else {
                                        Err(Err(error))
                                    }
                                }
                                result => Err(result.map_err(|(e, _)| e)),
                            }
                        };
                        let _guard = flag.lock().unwrap_or_else(|e| e.into_inner());
                        let mut state = queue.lock().unwrap_or_else(|e| e.into_inner());
                        state.1 -= 1;
                        match retry {
                            Ok((error, ready)) => state
                                .0
                                .push_back((position, request, sends, ready, first, error)),
                            Err(result) => {
                                let _ = tx.send((position, request, result));
                            }
                        }
                        wake.notify_all();
                    }
                });
            }
            drop(tx);
            for (position, request, result) in rx {
                on_result(position, request, result);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct AdvancingCodec {
        url: String,
        elapsed: Arc<AtomicU64>,
    }

    impl Codec for AdvancingCodec {
        type Request = ();
        type Response = ();

        fn encode(&self, _: &()) -> Result<ureq::http::Request<Vec<u8>>, Error> {
            self.elapsed.store(91, Ordering::SeqCst);
            crate::post_json(&self.url, None, &[], &serde_json::json!({}))
        }

        fn decode(&self, _: &(), _: u16, _: &[u8]) -> Result<(), Error> {
            Ok(())
        }
    }

    struct QueueCodec {
        url: String,
        first_attempts: std::sync::atomic::AtomicUsize,
    }

    impl Codec for QueueCodec {
        type Request = u8;
        type Response = u8;

        fn encode(&self, _: &u8) -> Result<ureq::http::Request<Vec<u8>>, Error> {
            crate::post_json(&self.url, None, &[], &serde_json::json!({}))
        }

        fn decode(&self, request: &u8, _: u16, _: &[u8]) -> Result<u8, Error> {
            if *request == 1 && self.first_attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                Err(Error::Invalid("retry".into()))
            } else {
                Ok(*request)
            }
        }
    }

    struct PausingCodec {
        url: String,
        encodes: Arc<AtomicU64>,
    }
    impl Codec for PausingCodec {
        type Request = ();
        type Response = ();
        fn encode(&self, _: &()) -> Result<ureq::http::Request<Vec<u8>>, Error> {
            self.encodes.fetch_add(1, Ordering::SeqCst);
            crate::post_json(&self.url, None, &[], &serde_json::json!({}))
        }
        fn decode(&self, _: &(), _: u16, _: &[u8]) -> Result<(), Error> {
            Ok(())
        }
    }

    struct CancelEncoder(Cancel);
    impl Codec for CancelEncoder {
        type Request = ();
        type Response = ();
        fn encode(&self, _: &()) -> Result<ureq::http::Request<Vec<u8>>, Error> {
            self.0.cancel();
            crate::post_json("http://127.0.0.1:1", None, &[], &serde_json::json!({}))
        }
        fn decode(&self, _: &(), _: u16, _: &[u8]) -> Result<(), Error> {
            Ok(())
        }
    }

    #[test]
    fn cancellation_during_encoding_prevents_a_send() {
        let cancel = Cancel::default();
        let client = Client::new(CancelEncoder(cancel.clone()), Policy::default());
        assert!(matches!(
            client.send(&(), &cancel),
            Err(Error::Cancelled { sends: 0 })
        ));
    }

    struct BadEncoder(AtomicU64);
    impl Codec for BadEncoder {
        type Request = ();
        type Response = ();
        fn encode(&self, _: &()) -> Result<ureq::http::Request<Vec<u8>>, Error> {
            if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
                Err(Error::Invalid("encode".into()))
            } else {
                Err(Error::Config("unexpected retry".into()))
            }
        }
        fn decode(&self, _: &(), _: u16, _: &[u8]) -> Result<(), Error> {
            Ok(())
        }
    }

    #[test]
    fn encoding_failure_is_not_requeued() {
        let client = Client::new(BadEncoder(AtomicU64::new(0)), Policy::default());
        assert!(matches!(
            client.send(&(), &Cancel::default()),
            Err(Error::Invalid(_))
        ));
        assert_eq!(client.codec().0.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn cancellation_ends_a_busy_retry_wait() {
        use std::sync::atomic::AtomicBool;
        let server = crate::test_server::Server::start(vec![(
            503,
            vec![("retry-after", "3600".into())],
            "{}".into(),
        )]);
        let start = Instant::now();
        let encodes = Arc::new(AtomicU64::new(0));
        let cancel = Cancel::default();
        let client = Client::new(
            PausingCodec {
                url: server.base.clone(),
                encodes: encodes.clone(),
            },
            Policy::default(),
        );
        let before_send = AtomicBool::new(false);
        let cancelled_once = AtomicBool::new(false);
        let mut answer = None;
        client.dispatch(
            vec![()],
            1,
            &cancel,
            || {
                if encodes.load(Ordering::SeqCst) > 0
                    && before_send.swap(true, Ordering::SeqCst)
                    && !cancelled_once.swap(true, Ordering::SeqCst)
                {
                    cancel.cancel();
                }
                start
            },
            |_, _, result| answer = Some(result),
        );
        assert!(matches!(answer, Some(Err(Error::Cancelled { sends: 1 }))));
        assert_eq!(server.finish().len(), 1);
    }

    #[test]
    fn pause_started_during_encoding_defers_transmission() {
        use std::sync::atomic::AtomicBool;
        let server = crate::test_server::Server::start(vec![(200, vec![], "{}".into())]);
        let start = Instant::now();
        let encodes = Arc::new(AtomicU64::new(0));
        let client = Client::new(
            PausingCodec {
                url: server.base.clone(),
                encodes: encodes.clone(),
            },
            Policy::default(),
        );
        let paused = AtomicBool::new(false);
        let mut answer = None;
        client.dispatch(
            vec![()],
            1,
            &Cancel::default(),
            || {
                if encodes.load(Ordering::SeqCst) == 0 {
                    start
                } else if !paused.swap(true, Ordering::SeqCst) {
                    client.pause(Duration::from_secs(3), start);
                    start
                } else {
                    start + Duration::from_secs(3)
                }
            },
            |_, _, result| answer = Some(result),
        );
        assert!(matches!(answer, Some(Ok(()))));
        assert_eq!(encodes.load(Ordering::SeqCst), 2);
        assert_eq!(server.finish().len(), 1);
    }

    #[test]
    fn oversized_response_is_not_retryable() {
        let error = body_error(ureq::Error::BodyExceedsLimit(MAX_BODY));
        assert!(matches!(error, Error::ResponseTooLarge { limit: MAX_BODY }));
        assert_eq!(error.id(), "http.response_too_large");
        assert!(!error.retryable());
    }

    #[test]
    fn held_request_leaves_worker_free_for_ready_work() {
        let server = crate::test_server::Server::start(vec![(200, vec![], "{}".into()); 3]);
        let start = Instant::now();
        let elapsed = AtomicU64::new(0);
        let cancel = Cancel::default();
        let client = Client::new(
            QueueCodec {
                url: server.base.clone(),
                first_attempts: Default::default(),
            },
            Policy::default(),
        );
        let mut results = Vec::new();
        client.dispatch(
            vec![1, 2],
            1,
            &cancel,
            || start + Duration::from_secs(elapsed.load(Ordering::SeqCst)),
            |_, request, result| {
                results.push((request, result.ok()));
                if request == 2 {
                    let _guard = cancel.0.0.lock().unwrap_or_else(|e| e.into_inner());
                    elapsed.store(2, Ordering::SeqCst);
                    cancel.0.1.notify_all();
                }
            },
        );
        assert_eq!(results, [(2, Some(2)), (1, Some(1))]);
        assert_eq!(server.finish().len(), 3);
    }

    #[test]
    fn first_send_budget_starts_after_encoding() {
        let server = crate::test_server::Server::start(vec![
            (503, vec![("retry-after", "0".into())], "{}".into()),
            (200, vec![], "{}".into()),
        ]);
        let start = Instant::now();
        let elapsed = Arc::new(AtomicU64::new(0));
        let client = Client::new(
            AdvancingCodec {
                url: server.base.clone(),
                elapsed: elapsed.clone(),
            },
            Policy::default(),
        );
        let mut answer = None;
        client.dispatch(
            vec![()],
            1,
            &Cancel::default(),
            || start + Duration::from_secs(elapsed.load(Ordering::SeqCst)),
            |_, _, result| answer = Some(result),
        );
        assert!(matches!(answer, Some(Ok(()))));
        assert_eq!(server.finish().len(), 2);
    }

    #[test]
    fn final_busy_response_sets_provider_pause_exactly() {
        let server = crate::test_server::Server::start(vec![(
            503,
            vec![("retry-after", "3".into())],
            "{}".into(),
        )]);
        let start = Instant::now();
        let client = Client::new(
            QueueCodec {
                url: server.base.clone(),
                first_attempts: Default::default(),
            },
            Policy {
                max_sends: 1,
                ..Policy::default()
            },
        );
        client.dispatch(vec![1], 1, &Cancel::default(), || start, |_, _, _| {});
        assert_eq!(client.not_before(), Some(start + Duration::from_secs(3)));
        assert_eq!(server.finish().len(), 1);
    }

    #[test]
    fn budget_stops_retry_after_a_busy_send_crosses_ninety_seconds() {
        let server = crate::test_server::Server::start(vec![(503, vec![], "{}".into())]);
        let start = Instant::now();
        let elapsed = Arc::new(AtomicU64::new(0));
        let client = Client::new(
            AdvancingCodec {
                url: server.base.clone(),
                elapsed: elapsed.clone(),
            },
            Policy::default(),
        );
        let mut answer = None;
        client.dispatch(
            vec![()],
            1,
            &Cancel::default(),
            || {
                let seconds = elapsed.load(Ordering::SeqCst);
                if seconds == 91 {
                    elapsed.store(182, Ordering::SeqCst);
                }
                start + Duration::from_secs(seconds)
            },
            |_, _, result| answer = Some(result),
        );
        assert!(matches!(
            answer,
            Some(Err(Error::ProviderBusy { attempts: 1, .. }))
        ));
        assert_eq!(server.finish().len(), 1);
    }

    #[test]
    fn cancellation_ends_a_queued_retry() {
        let server = crate::test_server::Server::start(vec![(200, vec![], "{}".into()); 2]);
        let start = Instant::now();
        let cancel = Cancel::default();
        let client = Client::new(
            QueueCodec {
                url: server.base.clone(),
                first_attempts: Default::default(),
            },
            Policy::default(),
        );
        let mut results = Vec::new();
        client.dispatch(
            vec![1, 2],
            1,
            &cancel,
            || start,
            |_, request, result| {
                if request == 2 {
                    cancel.cancel();
                }
                results.push((request, result));
            },
        );
        assert!(matches!(
            &results[..],
            [(2, Ok(2)), (1, Err(Error::Cancelled { sends: 1 }))]
        ));
        assert_eq!(server.finish().len(), 2);
    }

    #[test]
    fn budget_never_cuts_a_send_in_flight() {
        let server = crate::test_server::Server::start(vec![(200, vec![], "{}".into())]);
        let start = Instant::now();
        let elapsed = Arc::new(AtomicU64::new(0));
        let client = Client::new(
            AdvancingCodec {
                url: server.base.clone(),
                elapsed: elapsed.clone(),
            },
            Policy::default(),
        );
        let mut answer = None;
        client.dispatch(
            vec![()],
            1,
            &Cancel::default(),
            || {
                let seconds = elapsed.load(Ordering::SeqCst);
                if seconds == 91 {
                    elapsed.store(182, Ordering::SeqCst);
                }
                start + Duration::from_secs(seconds)
            },
            |_, _, result| answer = Some(result),
        );
        assert!(matches!(answer, Some(Ok(()))));
        assert_eq!(server.finish().len(), 1);
    }
}
