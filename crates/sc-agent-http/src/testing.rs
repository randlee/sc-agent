//! Transport test doubles for consumers' tests (feature `testing`, not
//! default; PLAN-CRIT-010).
//!
//! [`RefusingTransport`] is the production [`UreqTransport`](crate::UreqTransport)
//! with a connector that refuses every connection by construction (no port is
//! bound and freed), recording the whole-send deadline ureq applies to each
//! attempt.
//!
//! Consumers take `sc-agent-http = { path = .., features = ["testing"] }` as
//! a dev-dependency only. Nothing here sleeps: time moves only through
//! [`ManualClock::advance`], and [`BlockingTransport`] waits on a `Condvar`
//! that the test releases.
//!
//! # Examples
//!
//! ```
//! use std::time::Duration;
//!
//! use sc_agent_http::testing::{ManualClock, ScriptedTransport};
//! use sc_agent_http::{Endpoint, HttpRequest, HttpResponse, Method, Transport};
//!
//! let clock = ManualClock::new();
//! let start = clock.now();
//! let transport = ScriptedTransport::responses(vec![HttpResponse::new(200, vec![], Vec::new())])
//!     .with_send_duration(clock.clone(), Duration::from_secs(2));
//! let request = HttpRequest::new(Method::Get, Endpoint::parse("https://h.test/")?, Vec::new());
//! assert_eq!(transport.send(&request)?.status(), 200);
//! assert_eq!(clock.now() - start, Duration::from_secs(2));
//! assert!(transport.send(&request).is_err()); // script exhausted
//! assert_eq!(transport.sends(), 2);
//! # Ok::<(), sc_agent_http::HttpError>(())
//! ```

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use crate::error::{HttpError, NetworkKind};
use crate::request::{HttpRequest, HttpResponse};
use crate::transport::{Timeouts, Transport};
use crate::ureq_transport::{UreqTransport, refusing};

/// Locks `mutex`, recovering a poisoned lock: every guarded value here is
/// changed in single steps, so it is consistent after a panic elsewhere.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The manual clock of sc-agent-types, re-exported: `now()` for `send_once`, advanced by
/// sends ([`ScriptedTransport::with_send_duration`]) or by the test.
pub use sc_agent_types::testing::ManualClock;

/// A transport that answers from a script, first entry first.
///
/// Every request is recorded before it is answered. When the script runs
/// out, `send` returns [`HttpError::Network`] of kind `Connect` ("script
/// exhausted"). Clones share the script and the record, so a test can keep
/// one clone and hand the other to a client.
#[derive(Clone, Default)]
pub struct ScriptedTransport {
    inner: Arc<Mutex<Script>>,
}

#[derive(Default)]
struct Script {
    replies: VecDeque<Result<HttpResponse, HttpError>>,
    requests: Vec<HttpRequest>,
    clock: Option<(ManualClock, Duration)>,
}

impl ScriptedTransport {
    /// A transport that returns `replies` in order.
    #[must_use]
    pub fn new(replies: Vec<Result<HttpResponse, HttpError>>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Script {
                replies: replies.into(),
                ..Script::default()
            })),
        }
    }

    /// A transport that answers each send with the next of `responses`.
    #[must_use]
    pub fn responses(responses: Vec<HttpResponse>) -> Self {
        Self::new(responses.into_iter().map(Ok).collect())
    }

    /// Every send advances `clock` by `per_send` before answering (a send
    /// that takes the full timeout).
    #[must_use]
    pub fn with_send_duration(self, clock: ManualClock, per_send: Duration) -> Self {
        lock(&self.inner).clock = Some((clock, per_send));
        self
    }

    /// Every request sent so far, in order.
    #[must_use]
    pub fn requests(&self) -> Vec<HttpRequest> {
        lock(&self.inner).requests.clone()
    }

    /// Number of sends so far.
    #[must_use]
    pub fn sends(&self) -> usize {
        lock(&self.inner).requests.len()
    }
}

impl Transport for ScriptedTransport {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError> {
        let mut script = lock(&self.inner);
        script.requests.push(request.clone());
        if let Some((clock, per_send)) = &script.clock {
            clock.advance(*per_send);
        }
        script.replies.pop_front().unwrap_or_else(|| {
            Err(HttpError::network(
                NetworkKind::Connect,
                1,
                "script exhausted",
            ))
        })
    }
}

impl fmt::Debug for ScriptedTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let script = lock(&self.inner);
        formatter
            .debug_struct("ScriptedTransport")
            .field("replies_left", &script.replies.len())
            .field("requests", &script.requests)
            .finish_non_exhaustive()
    }
}

/// Records every request, then forwards it to `T`.
pub struct RecordingTransport<T> {
    inner: T,
    requests: Arc<Mutex<Vec<HttpRequest>>>,
}

impl<T: Transport> RecordingTransport<T> {
    /// A recorder in front of `inner`.
    #[must_use]
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            requests: Arc::default(),
        }
    }

    /// Every request sent so far, in order.
    #[must_use]
    pub fn requests(&self) -> Vec<HttpRequest> {
        lock(&self.requests).clone()
    }
}

impl<T: Transport> Transport for RecordingTransport<T> {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError> {
        lock(&self.requests).push(request.clone());
        self.inner.send(request)
    }
}

impl<T> fmt::Debug for RecordingTransport<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecordingTransport")
            .field("requests", &lock(&self.requests).len())
            .finish_non_exhaustive()
    }
}

/// A send that blocks on a `Condvar` until the test releases it
/// (PLAN-CRIT-002 cancel tests). No sleep.
///
/// Every wait is bounded by [`BlockingTransport::FAILSAFE`] so a broken ordering fails the
/// test instead of hanging it: [`Self::wait_started`] returns `false` (after releasing every send),
/// and a send that is never released returns a network error. The failsafe is a guard,
/// never a measurement. Clones share one gate.
///
/// # Examples
///
/// ```
/// use std::thread;
///
/// use sc_agent_http::testing::BlockingTransport;
/// use sc_agent_http::{Endpoint, HttpRequest, HttpResponse, Method, Transport};
///
/// let transport = BlockingTransport::new(Ok(HttpResponse::new(200, vec![], Vec::new())));
/// let sender = transport.clone();
/// let request = HttpRequest::new(Method::Get, Endpoint::parse("https://h.test/")?, Vec::new());
/// let worker = thread::spawn(move || sender.send(&request).map(|r| r.status()));
/// assert!(transport.wait_started(1));
/// transport.release();
/// assert_eq!(worker.join().ok(), Some(Ok(200)));
/// # Ok::<(), sc_agent_http::HttpError>(())
/// ```
#[derive(Clone)]
pub struct BlockingTransport {
    gate: Arc<(Mutex<Gate>, Condvar)>,
}

struct Gate {
    reply: Result<HttpResponse, HttpError>,
    started: usize,
    released: bool,
}

impl BlockingTransport {
    /// The longest any wait of this transport lasts before it gives up.
    pub const FAILSAFE: Duration = Duration::from_secs(60);

    /// A transport whose every released send returns a clone of `reply`.
    #[must_use]
    pub fn new(reply: Result<HttpResponse, HttpError>) -> Self {
        Self {
            gate: Arc::new((
                Mutex::new(Gate {
                    reply,
                    started: 0,
                    released: false,
                }),
                Condvar::new(),
            )),
        }
    }

    /// Blocks until `sends` sends have entered `send()`; `true` when they did.
    ///
    /// `false` when fewer than `sends` sends started within [`Self::FAILSAFE`]; every blocked
    /// send is then released, so the run can end. Callers assert the result.
    #[must_use]
    pub fn wait_started(&self, sends: usize) -> bool {
        let (mutex, condvar) = &*self.gate;
        let (mut gate, timeout) = condvar
            .wait_timeout_while(lock(mutex), Self::FAILSAFE, |gate| gate.started < sends)
            .unwrap_or_else(PoisonError::into_inner);
        if timeout.timed_out() {
            gate.released = true;
            drop(gate);
            condvar.notify_all();
            return false;
        }
        true
    }

    /// Lets every blocked and later send return.
    pub fn release(&self) {
        let (mutex, condvar) = &*self.gate;
        lock(mutex).released = true;
        condvar.notify_all();
    }

    /// Number of sends that have entered `send()`.
    #[must_use]
    pub fn sends(&self) -> usize {
        lock(&self.gate.0).started
    }
}

impl Transport for BlockingTransport {
    fn send(&self, _request: &HttpRequest) -> Result<HttpResponse, HttpError> {
        let (mutex, condvar) = &*self.gate;
        let mut gate = lock(mutex);
        gate.started += 1;
        condvar.notify_all();
        let (gate, timeout) = condvar
            .wait_timeout_while(gate, Self::FAILSAFE, |gate| !gate.released)
            .unwrap_or_else(PoisonError::into_inner);
        if timeout.timed_out() {
            return Err(HttpError::network(
                NetworkKind::Timeout,
                1,
                "BlockingTransport was never released (test failsafe)",
            ));
        }
        gate.reply.clone()
    }
}

impl fmt::Debug for BlockingTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let gate = lock(&self.gate.0);
        formatter
            .debug_struct("BlockingTransport")
            .field("started", &gate.started)
            .field("released", &gate.released)
            .finish_non_exhaustive()
    }
}

/// The production [`UreqTransport`] with a connector that refuses every connection
/// (`io.ConnectionRefused`), so a refused connection needs no freed port.
///
/// Every send runs the real path (timeout validation, the per-request deadline, ureq, the
/// error classification); only the socket connect is replaced. [`Self::deadlines`] reports
/// the whole-send deadline ureq applied to each connection attempt: the transport's
/// [`Timeouts::request`], or the request's own [`HttpRequest::deadline`].
///
/// # Examples
///
/// ```
/// use std::time::Duration;
///
/// use sc_agent_http::testing::RefusingTransport;
/// use sc_agent_http::{Endpoint, HttpRequest, Method, NetworkKind, Timeouts, Transport, HttpError};
///
/// let transport = RefusingTransport::new(Timeouts::default());
/// let request = HttpRequest::new(Method::Get, Endpoint::parse_local("http://127.0.0.1:9/")?, Vec::new());
/// let error = transport.send(&request).expect_err("refused");
/// assert!(matches!(error, HttpError::Network { kind: NetworkKind::Connect, .. }));
/// assert_eq!(transport.deadlines(), [Some(Duration::from_secs(120))]);
/// # Ok::<(), sc_agent_http::HttpError>(())
/// ```
#[derive(Clone)]
pub struct RefusingTransport {
    inner: UreqTransport,
    seen: refusing::Seen,
}

impl RefusingTransport {
    /// A refusing transport with `timeouts`.
    #[must_use]
    pub fn new(timeouts: Timeouts) -> Self {
        let seen = refusing::Seen::default();
        Self {
            inner: UreqTransport::refusing(timeouts, Arc::clone(&seen)),
            seen,
        }
    }

    /// The whole-send deadline (ureq `timeout_global`) of each connection attempt so far.
    #[must_use]
    pub fn deadlines(&self) -> Vec<Option<Duration>> {
        lock(&self.seen).clone()
    }
}

impl Transport for RefusingTransport {
    fn ready(&self) -> Result<(), HttpError> {
        self.inner.ready()
    }

    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError> {
        self.inner.send(request)
    }
}

impl fmt::Debug for RefusingTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RefusingTransport")
            .field("attempts", &lock(&self.seen).len())
            .finish_non_exhaustive()
    }
}
