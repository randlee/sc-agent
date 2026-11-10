//! Shared test support: requests, replies, transports, a runner harness, failsafe waits.
//!
//! Nothing here sleeps or delays, and no helper measures time: every wait is on a forced
//! ordering event (a gate, a channel, a `Condvar`) and is bounded by [`FAILSAFE`], which
//! turns a hang into a failure and is never an assertion.

pub mod scripted_backend;

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use sc_agent_http::{HttpClient, HttpError, HttpRequest, HttpResponse, ResendPolicy, Transport};
use sc_agent_log::testing::{MemoryLog, MemoryRecord};
use sc_agent_log::{Repo, SessionIdentity, SessionLogger};
use sc_agent_types::CancelToken;
use sc_decision_runner::{DecisionRunner, RunnerConfig};
use sc_decision_schema::{Backend, Decision, DecisionRequest, DecisionResult, SystemOneRequest};

pub use scripted_backend::ScriptedBackend;

/// Failsafe for waits that must finish (REQ-QA-018: 2 s); never a measurement.
pub const FAILSAFE: Duration = Duration::from_secs(2);

pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A request for `model` with one noul question per id.
pub fn request_with(model: &str, questions: &[&str]) -> DecisionRequest {
    let questions: Vec<String> = questions
        .iter()
        .map(|id| format!(r#""{id}":{{"type":"noul"}}"#))
        .collect();
    let text = format!(
        r#"{{"model":"{model}","state":"s","questions":{{{}}}}}"#,
        questions.join(",")
    );
    DecisionRequest::try_from(SystemOneRequest::from_json(&text).expect("body")).expect("request")
}

/// A request for `model` with question `q`.
pub fn request(model: &str) -> DecisionRequest {
    request_with(model, &["q"])
}

/// Requests `m0`..`m{n-1}`.
pub fn requests(n: usize) -> Vec<DecisionRequest> {
    (0..n).map(|index| request(&format!("m{index}"))).collect()
}

/// The model named in a sent request body.
pub fn model_of(request: &HttpRequest) -> String {
    let body: serde_json::Value = serde_json::from_slice(request.body()).expect("json body");
    body["model"].as_str().expect("model").to_owned()
}

/// The input position of a request built by [`requests`] (`m7` -> 7).
pub fn index_of(request: &HttpRequest) -> usize {
    model_of(request)
        .trim_start_matches('m')
        .parse()
        .expect("m<index>")
}

pub const OK_BODY: &str = r#"{"model":"m","answers":{"q":{"type":"noul","noul":0.5}},"usage":{"input_tokens":1,"output_tokens":1}}"#;

/// 200 with a valid response.
pub fn ok() -> HttpResponse {
    HttpResponse::new(200, vec![], OK_BODY.as_bytes().to_vec())
}

/// 529 with `Retry-After: <seconds>` (0 is clamped to 250 ms + jitter by sc-agent-http).
pub fn busy(retry_after: &str) -> HttpResponse {
    HttpResponse::new(
        529,
        vec![("retry-after".to_owned(), retry_after.to_owned())],
        br#"{"type":"error","error":{"type":"overloaded_error","message":"busy"}}"#.to_vec(),
    )
}

type SendFn = dyn Fn(&HttpRequest) -> Result<HttpResponse, HttpError> + Send + Sync;

/// A transport of one closure; signals on a channel when it is dropped (every runner
/// thread holding the client has exited).
pub struct FnTransport {
    send: Box<SendFn>,
    dropped: Mutex<Option<Sender<()>>>,
}

impl FnTransport {
    pub fn new(
        send: impl Fn(&HttpRequest) -> Result<HttpResponse, HttpError> + Send + Sync + 'static,
    ) -> (Self, Receiver<()>) {
        let (signal, dropped) = mpsc::channel();
        (
            Self {
                send: Box::new(send),
                dropped: Mutex::new(Some(signal)),
            },
            dropped,
        )
    }
}

impl Transport for FnTransport {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError> {
        (self.send)(request)
    }
}

impl Drop for FnTransport {
    fn drop(&mut self) {
        if let Some(signal) = lock(&self.dropped).take() {
            let _ = signal.send(());
        }
    }
}

/// A gate sends wait at until the test opens it; counts the sends inside.
#[derive(Clone, Default)]
pub struct Gate {
    inner: Arc<(Mutex<(usize, bool)>, Condvar)>,
}

impl Gate {
    /// Called by a transport: waits until [`Self::open`] (failsafe-bounded).
    pub fn pass(&self) {
        let (mutex, condvar) = &*self.inner;
        let mut state = lock(mutex);
        state.0 += 1;
        condvar.notify_all();
        let (_state, timeout) = condvar
            .wait_timeout_while(state, FAILSAFE * 5, |(_, open)| !*open)
            .unwrap_or_else(PoisonError::into_inner);
        assert!(!timeout.timed_out(), "the gate was never opened");
    }

    /// Waits until `sends` sends are inside (or passed) the gate.
    pub fn wait_inside(&self, sends: usize) {
        let (mutex, condvar) = &*self.inner;
        let (state, timeout) = condvar
            .wait_timeout_while(lock(mutex), FAILSAFE, |(inside, _)| *inside < sends)
            .unwrap_or_else(PoisonError::into_inner);
        assert!(
            !timeout.timed_out(),
            "{sends} sends did not reach the gate (got {})",
            state.0
        );
    }

    pub fn inside(&self) -> usize {
        lock(&self.inner.0).0
    }

    pub fn open(&self) {
        let (mutex, condvar) = &*self.inner;
        lock(mutex).1 = true;
        condvar.notify_all();
    }
}

/// A runner over `backend` and `transport`, logging into one `MemoryLog`.
pub struct Harness {
    pub log: MemoryLog,
    pub runner: Arc<DecisionRunner>,
}

impl Harness {
    pub fn new(
        backend: Arc<dyn Backend>,
        transport: Arc<dyn Transport>,
        config: RunnerConfig,
        max_sends: u32,
    ) -> Self {
        let log = MemoryLog::new().expect("memory log");
        let policy = ResendPolicy::new(NonZeroU32::new(max_sends).expect("max_sends"));
        let http = HttpClient::new(transport, policy, log.general());
        let identity = SessionIdentity::new(
            Repo::local("test-host", "sc-agent").expect("repo"),
            "phase-a",
            "runner-test",
        )
        .expect("identity");
        let runner =
            DecisionRunner::new(backend, http, log.session(identity), log.general(), config);
        Self {
            log,
            runner: Arc::new(runner),
        }
    }

    /// [`Self::new`] over `clock` (`DecisionRunner::with_clock`): holdoffs and pauses fall due
    /// only when the test moves the clock.
    pub fn with_clock(
        backend: Arc<dyn Backend>,
        transport: Arc<dyn Transport>,
        config: RunnerConfig,
        max_sends: u32,
        clock: sc_agent_http::testing::ManualClock,
    ) -> Self {
        let log = MemoryLog::new().expect("memory log");
        let policy = ResendPolicy::new(NonZeroU32::new(max_sends).expect("max_sends"));
        let http = HttpClient::new(transport, policy, log.general());
        let identity = SessionIdentity::new(
            Repo::local("test-host", "sc-agent").expect("repo"),
            "phase-a",
            "runner-test",
        )
        .expect("identity");
        let runner = DecisionRunner::with_clock(
            backend,
            http,
            log.session(identity),
            log.general(),
            config,
            Arc::new(move || clock.now()),
        );
        Self {
            log,
            runner: Arc::new(runner),
        }
    }

    /// Default backend, default config, 5 sends.
    pub fn of(transport: Arc<dyn Transport>) -> Self {
        Self::new(
            Arc::new(ScriptedBackend::new()),
            transport,
            RunnerConfig::default(),
            5,
        )
    }

    /// Session rows written so far.
    pub fn rows(&self) -> Vec<MemoryRecord> {
        self.log
            .records()
            .expect("records")
            .into_iter()
            .filter(|record| record.target == SessionLogger::TARGET)
            .collect()
    }

    /// General records of `target`.
    pub fn records_of(&self, target: &str) -> Vec<MemoryRecord> {
        self.log
            .records()
            .expect("records")
            .into_iter()
            .filter(|record| record.target == target)
            .collect()
    }

    /// Runs `decide_stream` on a helper thread; each result arrives on the channel.
    pub fn stream(
        &self,
        requests: Vec<DecisionRequest>,
        cancel: &CancelToken,
    ) -> Receiver<DecisionResult> {
        let runner = Arc::clone(&self.runner);
        let cancel = cancel.clone();
        let (send, results) = mpsc::channel();
        thread::Builder::new()
            .name("test-consumer".to_owned())
            .spawn(move || {
                for result in runner.decide_stream(Box::new(requests.into_iter()), &cancel) {
                    if send.send(result).is_err() {
                        return;
                    }
                }
            })
            .expect("consumer thread");
        results
    }

    /// Runs `decide` on a helper thread; the result arrives on the channel.
    pub fn decide(
        &self,
        request: DecisionRequest,
        cancel: &CancelToken,
    ) -> Receiver<DecisionResult> {
        let runner = Arc::clone(&self.runner);
        let cancel = cancel.clone();
        let (send, result) = mpsc::channel();
        thread::Builder::new()
            .name("test-caller".to_owned())
            .spawn(move || {
                let _ = send.send(runner.decide(request, &cancel));
            })
            .expect("caller thread");
        result
    }

    /// Drops the runner and waits for the transport to be dropped: every runner thread
    /// holding the client has exited. Rows read after this are final.
    pub fn finish(self, dropped: &Receiver<()>) -> MemoryLog {
        drop(self.runner);
        dropped
            .recv_timeout(FAILSAFE)
            .expect("every runner thread exited within the failsafe");
        self.log
    }
}

/// `n` results within the failsafe each.
pub fn take(results: &Receiver<DecisionResult>, n: usize) -> Vec<DecisionResult> {
    (0..n)
        .map(|index| {
            results
                .recv_timeout(FAILSAFE)
                .unwrap_or_else(|error| panic!("result {index} of {n} not received: {error}"))
        })
        .collect()
}

/// Every result until the stream ends, each within the failsafe.
pub fn drain(results: &Receiver<DecisionResult>) -> Vec<DecisionResult> {
    let mut all = Vec::new();
    loop {
        match results.recv_timeout(FAILSAFE) {
            Ok(result) => all.push(result),
            Err(mpsc::RecvTimeoutError::Disconnected) => return all,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                panic!(
                    "the stream did not end within the failsafe ({} results)",
                    all.len()
                )
            }
        }
    }
}

/// A config of `workers` workers (capacity 4 x workers).
pub fn config(workers: usize) -> RunnerConfig {
    RunnerConfig::new(workers).expect("workers in 1..=64")
}

/// Session rows of `log`.
pub fn rows_of(log: &MemoryLog) -> Vec<MemoryRecord> {
    log.records()
        .expect("records")
        .into_iter()
        .filter(|record| record.target == SessionLogger::TARGET)
        .collect()
}

/// Rows by session id.
pub fn rows_by_session(rows: &[MemoryRecord]) -> HashMap<String, Vec<&MemoryRecord>> {
    let mut by_session: HashMap<String, Vec<&MemoryRecord>> = HashMap::new();
    for row in rows {
        let session = row.fields["session_id"]
            .as_str()
            .expect("session_id")
            .to_owned();
        by_session.entry(session).or_default().push(row);
    }
    by_session
}

/// The `&dyn Decision` view (compile check of object use).
pub fn as_decision(runner: &DecisionRunner) -> &dyn Decision {
    runner
}

/// An input iterator that yields its first `ready` requests, then reports on a channel that
/// it is blocked and waits (bounded) for the test's signal before yielding the rest. While
/// it waits, the feeder thread is certainly alive and every earlier request was submitted.
pub struct GatedInput {
    requests: std::vec::IntoIter<DecisionRequest>,
    ready: usize,
    pulled: usize,
    blocked: Sender<()>,
    open: Receiver<()>,
}

impl GatedInput {
    /// The iterator, the "blocked" receiver and the "go on" sender.
    pub fn new(requests: Vec<DecisionRequest>, ready: usize) -> (Self, Receiver<()>, Sender<()>) {
        let (blocked_to, blocked) = mpsc::channel();
        let (open, open_from) = mpsc::channel();
        (
            Self {
                requests: requests.into_iter(),
                ready,
                pulled: 0,
                blocked: blocked_to,
                open: open_from,
            },
            blocked,
            open,
        )
    }
}

impl Iterator for GatedInput {
    type Item = DecisionRequest;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pulled == self.ready {
            let _ = self.blocked.send(());
            let _ = self.open.recv_timeout(FAILSAFE * 30);
        }
        self.pulled += 1;
        self.requests.next()
    }
}

/// An input iterator that reports on a channel when it has returned `None` (the input
/// ended): every request is then with the runner.
pub struct EndSignal {
    requests: std::vec::IntoIter<DecisionRequest>,
    ended: Option<Sender<()>>,
}

impl EndSignal {
    pub fn new(requests: Vec<DecisionRequest>) -> (Self, Receiver<()>) {
        let (ended, on_end) = mpsc::channel();
        (
            Self {
                requests: requests.into_iter(),
                ended: Some(ended),
            },
            on_end,
        )
    }
}

impl Iterator for EndSignal {
    type Item = DecisionRequest;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.requests.next();
        if next.is_none()
            && let Some(ended) = self.ended.take()
        {
            let _ = ended.send(());
        }
        next
    }
}
