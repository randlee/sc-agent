use std::time::{Duration, Instant};

use crate::{
    Cancel, Codec,
    holdoff::{self, Attempt, ProviderPause},
};
use sc_agent_types::Error;

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
    not_before: ProviderPause,
}

/// Parses Retry-After seconds into a duration.
pub fn retry_after(text: &str) -> Option<Duration> {
    Duration::try_from_secs_f64(text.parse::<f64>().ok()?).ok()
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
            not_before: ProviderPause::default(),
        }
    }

    pub fn codec(&self) -> &C {
        &self.codec
    }

    /// Sends a request, retrying transient failures within the shared budget.
    pub fn send(&self, request: &C::Request, cancel: &Cancel) -> Result<C::Response, Error> {
        holdoff::run_one(
            request,
            self.policy.max_sends,
            &self.not_before,
            cancel,
            |request, attempt| self.attempt(request, attempt),
            |sends| Error::Cancelled { sends },
        )
    }

    /// Returns the current shared provider pause deadline.
    pub fn not_before(&self) -> Option<Instant> {
        self.not_before.not_before()
    }

    fn attempt(&self, request: &C::Request, attempt: u32) -> Attempt<C::Response, Error> {
        match self.once(request, attempt) {
            Ok(response) => Attempt::Done(Ok(response)),
            Err((error, _)) if !error.retryable() && !matches!(error, Error::ResponseParse(_)) => {
                Attempt::Done(Err(error))
            }
            Err((error, retry_after)) => {
                let busy = matches!(error, Error::ProviderBusy { .. });
                let delay = holdoff::delay(self.policy.backoff, attempt, retry_after);
                Attempt::Retry { error, delay, busy }
            }
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
        holdoff::run(
            requests,
            (workers, self.policy.max_sends),
            &self.not_before,
            cancel,
            |request, attempt| self.attempt(request, attempt),
            |sends| Error::Cancelled { sends },
            |position, request, result| {
                on_result(Item {
                    position,
                    session: ulid::Ulid::new(),
                    request,
                    result,
                })
            },
        );
    }
}
