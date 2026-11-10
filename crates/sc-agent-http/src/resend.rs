//! Admission, one send, and the busy resend schedule (REQ-SAG-0601,
//! REQ-SAG-0602, REQ-SAG-0012). Nothing here waits.
//!
//! The caller's queue drives a request through [`HttpClient`]:
//!
//! 1. [`HttpClient::may_start`] (fresh request, or a caller resend after
//!    [`Next::Done`]) or [`HttpClient::may_resend`] (a held [`Pending`]) checks
//!    `max_sends`, the 90 s start budget and the host's [`ProviderPause`], and
//!    gives a [`SendPermit`] or a not-before instant ([`Start::Wait`]).
//! 2. [`HttpClient::send_once`] consumes the permit, sends once and returns
//!    [`Next::Done`] (a final reply) or [`Next::ResendAt`] (busy or timed
//!    out: hold the request until `at`, then go back to step 1).
//!
//! The holdoff wait and its cancellation belong to the caller (the runner
//! waits on a `Condvar` to the earliest `at` and wakes on cancel).

use std::collections::HashMap;
use std::fmt;
use std::hash::{BuildHasher, RandomState};
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use sc_agent_log::{GeneralLogger, Level, LogContext};
use sc_agent_types::{ErrorDetail, ErrorId};
use serde_json::{Map, Value};

use crate::error::{BusyReason, HttpError, NetworkKind};
use crate::quota::is_quota_reply;
use crate::redact::redact;
use crate::reply::ProviderReply;
use crate::request::{HttpRequest, HttpResponse, SecretHeader};
use crate::retry_after::RetryAfter;
use crate::transport::Transport;

/// HTTP statuses that mean "busy, resend the same body".
const BUSY_STATUSES: [u16; 3] = [429, 502, 529];

/// First backoff holdoff when `Retry-After` is absent or malformed.
const BACKOFF_FIRST: Duration = Duration::from_secs(1);

/// Cap of the exponential backoff.
const BACKOFF_MAX: Duration = Duration::from_secs(30);

/// Largest jitter added to a holdoff.
const JITTER_MAX: Duration = Duration::from_millis(250);

/// Smallest honoured `Retry-After` (no `Retry-After: 0` storm, REQ-SAG-0012).
const RETRY_AFTER_MIN: Duration = Duration::from_millis(250);

/// Budget for starting sends, measured from the first send.
const BUDGET: Duration = Duration::from_secs(90);

/// Sends per request under [`ResendPolicy::default`], the first included.
const DEFAULT_MAX_SENDS: NonZeroU32 = match NonZeroU32::new(5) {
    Some(sends) => sends,
    None => NonZeroU32::MIN,
};

/// The `target` of every event this crate logs.
const LOG_TARGET: &str = "sc_agent_http";

/// Whether `status` is a busy status (429, 502, 529).
pub(crate) fn is_busy_status(status: u16) -> bool {
    BUSY_STATUSES.contains(&status)
}

/// Jitter in `0..=JITTER_MAX` (0-250 ms).
///
/// Hashes the current time with a fresh std `RandomState`: enough to
/// de-synchronise clients, not a cryptographic source.
fn jitter() -> Duration {
    let max = u64::try_from(JITTER_MAX.as_millis()).unwrap_or(u64::MAX);
    let hash = RandomState::new().hash_one(Instant::now());
    Duration::from_millis(hash % (max + 1))
}

/// Backoff before the `nth` resend (1-based): 1, 2, 4, 8 … s, capped at 30 s.
fn backoff(nth: u32) -> Duration {
    let factor = 1_u32.checked_shl(nth.saturating_sub(1)).unwrap_or(u32::MAX);
    BACKOFF_FIRST.saturating_mul(factor).min(BACKOFF_MAX)
}

/// The holdoff after the busy reply of send `sends`, and whether a valid
/// `Retry-After` set it.
///
/// A valid `Retry-After` is clamped to 250 ms-30 s and gets 0-250 ms jitter
/// (REQ-SAG-0012); without one the backoff is 1, 2, 4, 8, 16, 30 … s plus
/// 0-250 ms jitter (REQ-SAG-0601).
fn holdoff(response: &HttpResponse, sends: u32) -> (Duration, bool) {
    match RetryAfter::parse(response.header("retry-after")) {
        Some(retry_after) => (
            retry_after.wait().clamp(RETRY_AFTER_MIN, BACKOFF_MAX) + jitter(),
            true,
        ),
        None => (backoff(sends) + jitter(), false),
    }
}

/// How many sends one request may make (REQ-SAG-0601).
///
/// Any `NonZeroU32` is accepted; the 1..=10 bound of `max_sends` is checked
/// at config load (A-4).
///
/// # Examples
///
/// ```
/// use std::num::NonZeroU32;
///
/// use sc_agent_http::ResendPolicy;
///
/// assert_eq!(ResendPolicy::default().max_sends(), 5);
/// let two = NonZeroU32::new(2).ok_or("zero")?;
/// assert_eq!(ResendPolicy::new(two).max_sends(), 2);
/// # Ok::<(), &str>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResendPolicy {
    max_sends: NonZeroU32,
}

impl ResendPolicy {
    /// At most `max_sends` sends per request, the first included
    /// (`AgentConfig.max_sends`).
    #[must_use]
    pub const fn new(max_sends: NonZeroU32) -> Self {
        Self { max_sends }
    }

    /// Sends per request, the first included.
    #[must_use]
    pub const fn max_sends(self) -> u32 {
        self.max_sends.get()
    }
}

impl Default for ResendPolicy {
    /// 5 sends, the same value as `sc_agent_config::DEFAULT_MAX_SENDS` (this
    /// crate does not depend on config).
    fn default() -> Self {
        Self::new(DEFAULT_MAX_SENDS)
    }
}

/// Resend state of one request: sends made and when the first started.
///
/// A fresh request uses `Attempts::new()`; [`Next::Done`] hands it back for
/// a caller resend after a final reply (a-9 schema mismatch), so that resend
/// counts toward the same `max_sends` and budget.
#[derive(Debug, Clone, Default)]
pub struct Attempts {
    first_send: Option<Instant>,
    sends: u32,
}

impl Attempts {
    /// No send made yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sends made so far.
    #[must_use]
    pub fn sends(&self) -> u32 {
        self.sends
    }

    /// Why no further send may start at `now`, if a limit is reached.
    fn refusal(&self, policy: ResendPolicy, now: Instant) -> Option<BusyReason> {
        if self.sends >= policy.max_sends() {
            return Some(BusyReason::Sends);
        }
        match self.first_send {
            Some(first) if now.saturating_duration_since(first) >= BUDGET => {
                Some(BusyReason::Budget)
            }
            _ => None,
        }
    }
}

/// A request held for resend after a busy reply or a timeout.
///
/// Only [`HttpClient::send_once`] makes one, so the reason it is held (and
/// the error it ends with) is always known.
#[derive(Debug, Clone)]
pub struct Pending {
    attempts: Attempts,
    last: Last,
}

/// Why a [`Pending`] is held.
#[derive(Debug, Clone)]
enum Last {
    /// The last reply was busy (already redacted).
    Busy(ProviderReply),
    /// The last send timed out (message and cause already redacted).
    Timeout {
        message: String,
        cause: Option<Box<ErrorDetail>>,
    },
}

impl Last {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Busy(_) => "busy",
            Self::Timeout { .. } => "timeout",
        }
    }
}

impl Pending {
    /// Sends made so far and when the first started.
    #[must_use]
    pub fn attempts(&self) -> &Attempts {
        &self.attempts
    }

    /// The error this request ends with when `reason` refuses its resend.
    fn into_error(self, reason: BusyReason) -> HttpError {
        let attempts = self.attempts.sends;
        match self.last {
            Last::Busy(reply) => HttpError::ProviderBusy {
                reason,
                attempts,
                retry_after_seconds: reply.retry_after_seconds,
                last: Box::new(reply),
            },
            Last::Timeout { message, cause } => {
                let error = HttpError::network(NetworkKind::Timeout, attempts, message);
                match cause {
                    Some(cause) => error.with_cause(*cause),
                    None => error,
                }
            }
        }
    }
}

/// Leave to make one send now.
///
/// Only [`HttpClient::may_start`] / [`HttpClient::may_resend`] make one, and
/// [`HttpClient::send_once`] consumes it, so every send passed the
/// `max_sends`, budget and provider-pause checks. It is not `Clone` and has
/// no public constructor:
///
/// ```compile_fail,E0382
/// use sc_agent_http::{HttpClient, HttpRequest, LogContext, SendPermit};
///
/// fn send_twice(client: &HttpClient, request: &HttpRequest, permit: SendPermit) {
///     let now = std::time::Instant::now;
///     let _ = client.send_once(request, permit, LogContext::NONE, &now);
///     let _ = client.send_once(request, permit, LogContext::NONE, &now);
/// }
/// ```
///
/// ```compile_fail,E0451
/// use sc_agent_http::{Attempts, SendPermit};
///
/// let _forged = SendPermit { attempts: Attempts::new(), last: None };
/// ```
#[derive(Debug)]
#[must_use]
pub struct SendPermit {
    attempts: Attempts,
    last: Option<Last>,
}

/// Outcome of an admission check that did not refuse.
#[derive(Debug)]
#[must_use]
pub enum Start<S> {
    /// Send now with this permit.
    Go(SendPermit),
    /// The host is paused (REQ-SAG-0012): hold `state` and ask again no
    /// earlier than `at`.
    Wait {
        /// When to ask again.
        at: Instant,
        /// The request's state, handed back unchanged.
        state: S,
    },
}

/// What one send gave.
#[derive(Debug)]
#[must_use]
pub enum Next {
    /// A final reply (any status that is neither busy nor quota). The
    /// backend decodes it.
    Done {
        /// The reply, unchanged.
        response: HttpResponse,
        /// Sends made, for a caller resend through [`HttpClient::may_start`].
        attempts: Attempts,
    },
    /// Busy or timed out: hold `pending`, call [`HttpClient::may_resend`] no
    /// earlier than `at`, and send the request unchanged.
    ResendAt {
        /// Not before this instant.
        at: Instant,
        /// The held request's state.
        pending: Pending,
    },
}

/// Per-host "not before" shared by every clone of one [`HttpClient`]
/// (REQ-SAG-0012).
///
/// Keys are [`Endpoint::host`](crate::Endpoint::host); a busy reply moves the
/// host's time later, never earlier. Entries in the past are ignored (the map
/// is bounded by the number of hosts). Every access is one insert or one read
/// with no call out while locked, so the map is consistent after a panic
/// elsewhere and a poisoned lock is recovered.
#[derive(Debug, Clone, Default)]
pub struct ProviderPause {
    hosts: Arc<Mutex<HashMap<String, Instant>>>,
}

impl ProviderPause {
    fn hosts(&self) -> MutexGuard<'_, HashMap<String, Instant>> {
        self.hosts.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The instant before which no send to `host` may start, if a busy reply
    /// set one (it may be in the past).
    #[must_use]
    pub fn not_before(&self, host: &str) -> Option<Instant> {
        self.hosts().get(host).copied()
    }

    /// Pauses `host` until `until`, keeping the later instant.
    pub(crate) fn hold_until(&self, host: &str, until: Instant) {
        let mut hosts = self.hosts();
        match hosts.get_mut(host) {
            Some(held) => *held = (*held).max(until),
            None => {
                hosts.insert(host.to_owned(), until);
            }
        }
    }

    /// `Some(at)` when `host` is paused past `now`.
    fn paused_at(&self, host: &str, now: Instant) -> Option<Instant> {
        self.not_before(host).filter(|at| *at > now)
    }
}

/// Sends requests one at a time under a [`ResendPolicy`], with a shared
/// [`ProviderPause`] and the caller's [`GeneralLogger`]. Clones share the
/// transport, the pause and the logger.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use std::time::Instant;
///
/// use sc_agent_http::testing::ScriptedTransport;
/// use sc_agent_http::{
///     Attempts, Endpoint, HttpClient, HttpRequest, HttpResponse, LogContext, Method, Next,
///     ResendPolicy, Start,
/// };
/// use sc_agent_log::testing::MemoryLog;
///
/// let transport = ScriptedTransport::responses(vec![HttpResponse::new(200, vec![], b"{}".to_vec())]);
/// let memory = MemoryLog::new()?;
/// let client = HttpClient::new(Arc::new(transport), ResendPolicy::default(), memory.general());
/// let request = HttpRequest::new(Method::Post, Endpoint::parse("https://api.example.test/v1")?, b"{}".as_slice());
///
/// let start = client.may_start(&request, Attempts::new(), Instant::now()).map_err(|reason| reason.as_str())?;
/// let Start::Go(permit) = start else {
///     return Err("a fresh request to an unpaused host may start".into());
/// };
/// let Next::Done { response, .. } = client.send_once(&request, permit, LogContext::NONE, &Instant::now)? else {
///     return Err("200 is a final reply".into());
/// };
/// assert_eq!(response.status(), 200);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone)]
pub struct HttpClient {
    transport: Arc<dyn Transport>,
    policy: ResendPolicy,
    pause: ProviderPause,
    log: GeneralLogger,
}

impl fmt::Debug for HttpClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpClient")
            .field("policy", &self.policy)
            .field("pause", &self.pause)
            .finish_non_exhaustive()
    }
}

impl HttpClient {
    /// A client sending through `transport` under `policy`; `log` is the
    /// caller's general logger, default or injected (REQ-SAG-0008).
    #[must_use]
    pub fn new(transport: Arc<dyn Transport>, policy: ResendPolicy, log: GeneralLogger) -> Self {
        Self {
            transport,
            policy,
            pause: ProviderPause::default(),
            log,
        }
    }

    /// The resend policy.
    #[must_use]
    pub fn policy(&self) -> ResendPolicy {
        self.policy
    }

    /// The per-host pause shared by every clone of this client.
    #[must_use]
    pub fn pause(&self) -> &ProviderPause {
        &self.pause
    }

    /// The transport, for one-shot probes (A-6 discovery).
    #[must_use]
    pub fn transport(&self) -> &Arc<dyn Transport> {
        &self.transport
    }

    /// Admission of a first send, or of a caller resend after
    /// [`Next::Done`].
    ///
    /// # Errors
    ///
    /// [`BusyReason::Sends`] when `attempts` already made `max_sends` sends;
    /// [`BusyReason::Budget`] when the first send started 90 s or more before
    /// `now`. No [`HttpError`] exists for these: the caller finishes with the
    /// result it has.
    pub fn may_start(
        &self,
        request: &HttpRequest,
        attempts: Attempts,
        now: Instant,
    ) -> Result<Start<Attempts>, BusyReason> {
        if let Some(reason) = attempts.refusal(self.policy, now) {
            return Err(reason);
        }
        if let Some(at) = self.pause.paused_at(request.endpoint().host(), now) {
            return Ok(Start::Wait {
                at,
                state: attempts,
            });
        }
        Ok(Start::Go(SendPermit {
            attempts,
            last: None,
        }))
    }

    /// Admission of a resend of a held request.
    ///
    /// # Errors
    ///
    /// When `max_sends` or the 90 s start budget refuses the resend, the
    /// error the request ends with: last reply busy ->
    /// [`HttpError::ProviderBusy`] with the last reply; last send timed out
    /// -> [`HttpError::Network`] of kind timeout (never `ProviderBusy`).
    pub fn may_resend(
        &self,
        request: &HttpRequest,
        pending: Pending,
        now: Instant,
    ) -> Result<Start<Pending>, HttpError> {
        if let Some(reason) = pending.attempts.refusal(self.policy, now) {
            return Err(redact(pending.into_error(reason), request));
        }
        if let Some(at) = self.pause.paused_at(request.endpoint().host(), now) {
            return Ok(Start::Wait { at, state: pending });
        }
        Ok(Start::Go(SendPermit {
            attempts: pending.attempts,
            last: Some(pending.last),
        }))
    }

    /// One send under `permit`. Never waits.
    ///
    /// `now` is the caller's clock: read before the send (the first send
    /// starts the 90 s budget) and after it (the holdoff start). Every
    /// `http.*` event carries `ctx`. Every error is redacted of the request's
    /// key. Every busy reply that is not quota pauses the request's host
    /// ([`ProviderPause`], REQ-SAG-0012), the last allowed send's included.
    ///
    /// # Errors
    ///
    /// - [`HttpError::InvalidRequest`] before any send when the request's
    ///   deadline is zero or over [`Timeouts::MAX`](crate::Timeouts::MAX);
    /// - [`HttpError::QuotaExhausted`] for a busy status whose body marks
    ///   quota exhaustion, after this send (REQ-SAG-0602);
    /// - [`HttpError::ProviderBusy`] for a busy reply on the last allowed
    ///   send, or when the start budget has no time left;
    /// - [`HttpError::Network`] of kind timeout when the last allowed send
    ///   timed out; any other transport error after this send.
    pub fn send_once(
        &self,
        request: &HttpRequest,
        permit: SendPermit,
        ctx: LogContext,
        now: &dyn Fn() -> Instant,
    ) -> Result<Next, HttpError> {
        if let Some(deadline) = request.deadline() {
            crate::transport::check_duration("the request deadline", deadline)?;
        }
        let SendPermit { mut attempts, last } = permit;
        let max_sends = self.policy.max_sends();
        let attempt = attempts.sends.saturating_add(1);
        let endpoint = request.endpoint().log_form();
        self.event(
            Level::Info,
            "http.send",
            ctx,
            fields([
                ("method", request.method().as_str().into()),
                ("endpoint", endpoint.clone().into()),
                ("attempt", attempt.into()),
                ("max_sends", max_sends.into()),
                ("after", last.as_ref().map(Last::as_str).into()),
            ]),
        );
        let started = now();
        let timer = Instant::now();
        let result = self.transport.send(request);
        let elapsed_ms = millis(timer.elapsed());
        let ended = now();
        attempts.sends = attempt;
        let first_send = *attempts.first_send.get_or_insert(started);

        let response = match result {
            Ok(response) => response,
            Err(error) => {
                let error = redact(error.with_attempts(attempt), request);
                return self.transport_failed(error, attempts, ctx, elapsed_ms, ended);
            }
        };
        let status = response.status();
        let secret = request.secret().map(SecretHeader::key);
        // A busy reply is read whole (body or header request id); a final
        // reply only for its request-id header (the backend reads the body).
        let reply = is_busy_status(status).then(|| ProviderReply::read(&response, secret));
        let request_id = match &reply {
            Some(reply) => reply.request_id.clone(),
            None => response
                .header("request-id")
                .or_else(|| response.header("x-request-id"))
                .map(|id| match secret {
                    Some(key) => crate::redact::redact_owned(key, id.to_owned()),
                    None => id.to_owned(),
                }),
        };
        self.event(
            Level::Info,
            "http.response",
            ctx,
            fields([
                ("status", status.into()),
                ("attempt", attempt.into()),
                ("elapsed_ms", elapsed_ms.into()),
                ("provider_request_id", request_id.clone().into()),
            ]),
        );
        let Some(reply) = reply else {
            return Ok(Next::Done { response, attempts });
        };
        if is_quota_reply(status, response.body()) {
            self.event(
                Level::Warn,
                "http.quota_exhausted",
                ctx,
                fields([
                    ("status", status.into()),
                    ("attempt", attempt.into()),
                    ("endpoint", endpoint.into()),
                    ("provider_request_id", request_id.into()),
                ]),
            );
            return Err(HttpError::QuotaExhausted {
                attempts: attempt,
                reply: Box::new(reply),
            });
        }
        // Every non-quota busy reply pauses the host, whatever happens to
        // this request next (REQ-SAG-0012).
        let (holdoff, retry_after_honoured) = holdoff(&response, attempt);
        let host = request.endpoint().host();
        self.pause.hold_until(host, ended + holdoff);
        let host_paused_until_ms = self
            .pause
            .not_before(host)
            .map(|at| millis(at.saturating_duration_since(ended)));
        let busy_fields = |extra: [(&str, Value); 2]| {
            let mut map = fields([
                ("status", status.into()),
                ("attempt", attempt.into()),
                ("endpoint", endpoint.clone().into()),
                ("host", host.into()),
                ("provider_request_id", request_id.clone().into()),
                ("host_paused_until_ms", host_paused_until_ms.into()),
            ]);
            map.extend(extra.map(|(name, value)| (name.to_owned(), value)));
            map
        };
        let pending = Pending {
            attempts,
            last: Last::Busy(reply),
        };
        if attempt >= max_sends {
            self.event(
                Level::Warn,
                "http.busy_exhausted",
                ctx,
                busy_fields([
                    ("max_sends", max_sends.into()),
                    ("holdoff_ms", millis(holdoff).into()),
                ]),
            );
            return Err(pending.into_error(BusyReason::Sends));
        }
        let remaining = BUDGET.saturating_sub(ended.saturating_duration_since(first_send));
        if remaining.is_zero() {
            self.event(
                Level::Warn,
                "http.budget_exhausted",
                ctx,
                busy_fields([
                    ("budget_ms", millis(BUDGET).into()),
                    ("holdoff_ms", millis(holdoff).into()),
                ]),
            );
            return Err(pending.into_error(BusyReason::Budget));
        }
        self.event(
            Level::Warn,
            "http.busy",
            ctx,
            busy_fields([
                ("holdoff_ms", millis(holdoff).into()),
                ("retry_after_honoured", retry_after_honoured.into()),
            ]),
        );
        Ok(Next::ResendAt {
            at: ended + holdoff.min(remaining),
            pending,
        })
    }

    /// A transport error of send `attempts.sends`: a timeout with sends left
    /// is held for a resend after 0-250 ms of jitter, with no host pause
    /// (REQ-SAG-0601); anything else is returned now.
    fn transport_failed(
        &self,
        error: HttpError,
        attempts: Attempts,
        ctx: LogContext,
        elapsed_ms: u64,
        ended: Instant,
    ) -> Result<Next, HttpError> {
        let attempt = attempts.sends;
        let error = match error {
            HttpError::Network {
                kind: NetworkKind::Timeout,
                message,
                cause,
                ..
            } if attempt < self.policy.max_sends() => {
                let holdoff = jitter();
                self.event(
                    Level::Warn,
                    "http.timeout_resend",
                    ctx,
                    fields([
                        ("attempt", attempt.into()),
                        ("elapsed_ms", elapsed_ms.into()),
                        ("holdoff_ms", millis(holdoff).into()),
                    ]),
                );
                return Ok(Next::ResendAt {
                    at: ended + holdoff,
                    pending: Pending {
                        attempts,
                        last: Last::Timeout { message, cause },
                    },
                });
            }
            other => other,
        };
        self.event(
            Level::Warn,
            "http.transport_failed",
            ctx,
            fields([
                ("error", error.id().into()),
                ("attempt", attempt.into()),
                ("elapsed_ms", elapsed_ms.into()),
            ]),
        );
        Err(error)
    }

    /// Writes one event. A log error never changes a send result: the
    /// logger counts it and reports it on its next flush.
    fn event(
        &self,
        level: Level,
        action: &'static str,
        ctx: LogContext,
        fields: Map<String, Value>,
    ) {
        let _counted_by_the_logger = self.log.write(level, LOG_TARGET, action, ctx, fields);
    }
}

/// Event fields from `(name, value)` pairs, in order.
fn fields<const N: usize>(pairs: [(&str, Value); N]) -> Map<String, Value> {
    pairs
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect()
}

/// Whole milliseconds of `duration`, saturating.
fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retry_after::WAIT_MAX as RETRY_AFTER_MAX;

    #[test]
    fn backoff_doubles_to_the_cap() {
        let seconds: Vec<u64> = (1..=8).map(|nth| backoff(nth).as_secs()).collect();
        assert_eq!(seconds, [1, 2, 4, 8, 16, 30, 30, 30]);
        assert_eq!(backoff(u32::MAX), BACKOFF_MAX);
        assert_eq!(backoff(0), BACKOFF_FIRST);
    }

    #[test]
    fn retry_after_parser() {
        let cases: &[(Option<&str>, Option<Duration>)] = &[
            (None, None),
            (Some("3"), Some(Duration::from_secs(3))),
            (Some("2.5"), Some(Duration::from_millis(2_500))),
            (Some("0"), Some(Duration::ZERO)),
            (Some("0.000000001"), Some(Duration::from_nanos(1))),
            (Some("1.1234567899"), Some(Duration::new(1, 123_456_789))),
            (Some("29.999"), Some(Duration::from_millis(29_999))),
            (Some("30"), Some(RETRY_AFTER_MAX)),
            (Some("30.5"), Some(RETRY_AFTER_MAX)),
            (Some(&"9".repeat(32)), Some(RETRY_AFTER_MAX)),
            (Some(&"9".repeat(33)), None),
            (Some(""), None),
            (Some(" 7"), None),
            (Some("-1"), None),
            (Some("+1"), None),
            (Some("1e9"), None),
            (Some("inf"), None),
            (Some("NaN"), None),
            (Some("1."), None),
            (Some(".5"), None),
            (Some("1.2.3"), None),
            (Some("Wed, 21 Oct 2026 07:28:00 GMT"), None),
        ];
        for (header, expected) in cases {
            assert_eq!(
                RetryAfter::parse(*header).map(RetryAfter::wait),
                *expected,
                "{header:?}"
            );
        }
    }

    #[test]
    fn system_jitter_stays_in_range() {
        for _ in 0..64 {
            assert!(jitter() <= JITTER_MAX);
        }
    }

    #[test]
    fn default_delays_are_one_two_four_eight() {
        let delays: Vec<u64> = (1..ResendPolicy::default().max_sends())
            .map(|nth| backoff(nth).as_secs())
            .collect();
        assert_eq!(delays, [1, 2, 4, 8]);
        assert_eq!(delays.iter().sum::<u64>(), 15);
    }

    #[test]
    fn busy_set_is_exactly_three_statuses() {
        let busy: Vec<u16> = (100..600)
            .filter(|status| is_busy_status(*status))
            .collect();
        assert_eq!(busy, [429, 502, 529]);
    }

    #[test]
    fn pause_keeps_the_later_instant() {
        let pause = ProviderPause::default();
        let now = Instant::now();
        pause.hold_until("h", now + Duration::from_secs(5));
        pause.hold_until("h", now + Duration::from_secs(1));
        assert_eq!(pause.not_before("h"), Some(now + Duration::from_secs(5)));
        assert_eq!(pause.paused_at("h", now + Duration::from_secs(5)), None);
        assert_eq!(pause.not_before("other"), None);
    }
}
