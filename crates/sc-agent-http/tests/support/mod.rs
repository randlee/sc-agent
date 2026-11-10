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
