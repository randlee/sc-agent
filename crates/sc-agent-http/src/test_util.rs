//! Test doubles (feature `test-util`; plan "Test seams").
//!
//! - [`FakeClock`] and [`RecordingSleeper`] drive
//!   [`BusyPolicy::with_test_hooks`](crate::BusyPolicy::with_test_hooks), so
//!   retry tests never sleep.
//! - [`ScriptedTransport`] answers from a script and records every request.
//! - [`RecordingTransport`] records every request and forwards it to an inner
//!   transport.
//!
//! Neither transport can read a secret: a recorded [`HttpRequest`] exposes
//! only [`SecretHeader::redacted`](crate::SecretHeader::redacted) and
//! [`SecretHeader::matches`](crate::SecretHeader::matches).

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use sc_agent_types::{CancelToken, JevError, NetworkErrorKind};

use crate::request::{HttpRequest, HttpResponse};
use crate::transport::Transport;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A monotonic clock that moves only when told to.
///
/// Clones share one time value.
#[derive(Debug, Clone, Default)]
pub struct FakeClock(Arc<Mutex<Duration>>);

impl FakeClock {
    /// A clock at zero.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Time since the clock was created.
    #[must_use]
    pub fn now(&self) -> Duration {
        *lock(&self.0)
    }

    /// Moves the clock forward by `by`.
    pub fn advance(&self, by: Duration) {
        let mut now = lock(&self.0);
        *now = now.saturating_add(by);
    }
}

#[derive(Debug, Default)]
struct SleeperState {
    waits: Vec<Duration>,
    slices: Vec<Duration>,
    slept: Duration,
    cancel: Option<(CancelToken, Duration)>,
}

/// Records busy-retry waits instead of sleeping, and advances a [`FakeClock`].
///
/// Clones share one record.
#[derive(Clone)]
pub struct RecordingSleeper {
    clock: FakeClock,
    state: Arc<Mutex<SleeperState>>,
}

impl RecordingSleeper {
    /// A sleeper that advances `clock` by every slice it sleeps.
    #[must_use]
    pub fn new(clock: FakeClock) -> Self {
        Self {
            clock,
            state: Arc::default(),
        }
    }

    /// Cancels `token` once the total slept time reaches `after`.
    #[must_use]
    pub fn with_cancel_after(self, token: CancelToken, after: Duration) -> Self {
        lock(&self.state).cancel = Some((token, after));
        self
    }

    /// Each requested wait (jitter and budget cap applied), in order.
    #[must_use]
    pub fn waits(&self) -> Vec<Duration> {
        lock(&self.state).waits.clone()
    }

    /// Each slept slice, in order. No slice exceeds 100 ms.
    #[must_use]
    pub fn slices(&self) -> Vec<Duration> {
        lock(&self.state).slices.clone()
    }

    /// Total time slept.
    #[must_use]
    pub fn slept(&self) -> Duration {
        lock(&self.state).slept
    }

    pub(crate) fn record_wait(&self, wait: Duration) {
        lock(&self.state).waits.push(wait);
    }

    pub(crate) fn sleep(&self, slice: Duration) {
        self.clock.advance(slice);
        let mut state = lock(&self.state);
        state.slices.push(slice);
        state.slept = state.slept.saturating_add(slice);
        if let Some((token, after)) = &state.cancel
            && state.slept >= *after
        {
            token.cancel();
        }
    }
}

impl fmt::Debug for RecordingSleeper {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = lock(&self.state);
        formatter
            .debug_struct("RecordingSleeper")
            .field("clock", &self.clock)
            .field("waits", &state.waits)
            .field("slept", &state.slept)
            .finish_non_exhaustive()
    }
}

/// A transport that answers from a script, first entry first.
///
/// Every request is recorded before it is answered. When the script runs
/// out, `send` returns `NETWORK_ERROR` kind `Connect` ("script exhausted").
/// Clones share the script and the record, so a test can keep one clone and
/// hand the other to a backend.
#[derive(Clone, Default)]
pub struct ScriptedTransport {
    inner: Arc<Mutex<Script>>,
}

#[derive(Default)]
struct Script {
    replies: VecDeque<Result<HttpResponse, JevError>>,
    requests: Vec<HttpRequest>,
    clock: Option<(FakeClock, Duration)>,
}

impl ScriptedTransport {
    /// A transport that returns `replies` in order.
    #[must_use]
    pub fn new(replies: Vec<Result<HttpResponse, JevError>>) -> Self {
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

    /// Advances `clock` by `per_send` on every send (a slow server).
    #[must_use]
    pub fn with_send_duration(self, clock: FakeClock, per_send: Duration) -> Self {
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
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, JevError> {
        let mut script = lock(&self.inner);
        script.requests.push(request.clone());
        if let Some((clock, per_send)) = &script.clock {
            clock.advance(*per_send);
        }
        script.replies.pop_front().unwrap_or_else(|| {
            Err(JevError::network(
                NetworkErrorKind::Connect,
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
///
/// Clones share the record.
#[derive(Clone)]
pub struct RecordingTransport<T> {
    inner: T,
    requests: Arc<Mutex<Vec<HttpRequest>>>,
}

impl<T: Transport> RecordingTransport<T> {
    /// Wraps `inner`.
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
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, JevError> {
        lock(&self.requests).push(request.clone());
        self.inner.send(request)
    }
}

impl<T: fmt::Debug> fmt::Debug for RecordingTransport<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecordingTransport")
            .field("inner", &self.inner)
            .field("requests", &lock(&self.requests))
            .finish()
    }
}
