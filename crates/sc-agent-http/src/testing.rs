//! Transport test doubles for consumers' tests (feature `testing`, not
//! default; PLAN-CRIT-010).
//!
//! Consumers take `sc-agent-http = { path = .., features = ["testing"] }` as
//! a dev-dependency only. Nothing here sleeps: time moves only through
//! [`ManualClock::advance`], and [`BlockingTransport`] waits on a `Condvar`
//! that the test releases.
//!
//! # Examples
//!
//! ```
//! use std::time::{Duration, Instant};
//!
//! use sc_agent_http::testing::{ManualClock, ScriptedTransport};
//! use sc_agent_http::{Endpoint, HttpRequest, HttpResponse, Method, Transport};
//!
//! let clock = ManualClock::new(Instant::now());
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
use std::time::{Duration, Instant};

use crate::error::{HttpError, NetworkKind};
use crate::request::{HttpRequest, HttpResponse};
use crate::transport::Transport;

/// Locks `mutex`, recovering a poisoned lock: every guarded value here is
/// changed in single steps, so it is consistent after a panic elsewhere.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Explicit test clock shared with a transport: `now()` for `send_once`,
/// advanced by sends ([`ScriptedTransport::with_send_duration`]) or by the
/// test. Clones share one instant.
#[derive(Debug, Clone)]
pub struct ManualClock {
    now: Arc<Mutex<Instant>>,
}

impl ManualClock {
    /// A clock reading `start`.
    #[must_use]
    pub fn new(start: Instant) -> Self {
        Self {
            now: Arc::new(Mutex::new(start)),
        }
    }

    /// The current reading.
    #[must_use]
    pub fn now(&self) -> Instant {
        *lock(&self.now)
    }

    /// Moves the clock forward by `by`.
    pub fn advance(&self, by: Duration) {
        let mut now = lock(&self.now);
        *now += by;
    }
}

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
            Err(HttpError::Network {
                kind: NetworkKind::Connect,
                attempts: 1,
                message: "script exhausted".to_owned(),
            })
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
/// (PLAN-CRIT-002 cancel tests). No sleep, no timeout.
///
/// Clones share one gate.
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
/// transport.wait_started(1);
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

    /// Blocks until `sends` sends have entered `send()`.
    pub fn wait_started(&self, sends: usize) {
        let (mutex, condvar) = &*self.gate;
        let mut gate = lock(mutex);
        while gate.started < sends {
            gate = condvar.wait(gate).unwrap_or_else(PoisonError::into_inner);
        }
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
        while !gate.released {
            gate = condvar.wait(gate).unwrap_or_else(PoisonError::into_inner);
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
