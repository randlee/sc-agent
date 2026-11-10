//! [`DecisionRunner`]: `Decision` over any `Backend` and one `HttpClient`.
//!
//! One run (`decide` or `decide_stream`) has one feeder thread pulling the caller's
//! iterator into a bounded [`JobQueue`], `workers` worker threads, a bounded result queue
//! and one `CancelToken` hook. A job is [`Fresh`] until a worker encodes it once into a
//! [`Prepared`] job; only a prepared job is sent or held. Each send is admitted by the
//! `HttpClient` (`may_start` / `may_resend`) and made by `send_once`; a holdoff, a
//! provider pause and a schema resend all put the job back in the queue, never wait in
//! the worker.
//!
//! Cancel: the hook (registered before any thread starts) cancels the job queue and
//! interrupts the result queue. The result iterator then answers, at once and on the
//! consumer's thread, every queued and held job and every in-flight send (an orphan),
//! without waiting for a worker or the transport. The worker whose send later returns
//! loses the claim and discards the reply.

use std::collections::VecDeque;
use std::fmt;
use std::iter;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Instant;

use sc_agent_http::{Attempts, HttpClient, HttpRequest, Next, Pending, SendPermit, Start};
use sc_agent_log::{GeneralLogger, Level, LogContext, SessionLogger};
use sc_agent_types::{CancelToken, SessionId};
use sc_decision_schema::{
    Backend, Decision, DecisionError, DecisionRequest, DecisionResult, OrderedMap, QuestionDigest,
    SystemOneResponse,
};
use serde_json::{Map, Value};

use crate::config::RunnerConfig;
use crate::error::RunnerError;
use crate::pool::{Pool, Step, Stopped, Work};
use crate::queue::{BoundedQueue, Clock, JobQueue, Popped, Skipped};
use crate::row::{Claim, Finished, LOG_TARGET, Mode, Progress, Row, write_row};

/// The job queue of a run: jobs, with the request's [`Identity`] per in-flight send.
type RunnerJobs = JobQueue<Job, Identity>;
/// The result queue of a run.
type Results = BoundedQueue<Finished>;
/// The caller's requests.
type Requests = Box<dyn Iterator<Item = DecisionRequest> + Send + 'static>;

/// Runs decisions for one backend over one HTTP client, with session and general logs.
///
/// Implements [`Decision`]: `decide_stream` runs [`RunnerConfig::workers`] workers and
/// returns results fastest first, one per request, failures included; `decide` runs one
/// request on one runner worker. Every worker sends through the one `HttpClient`, so a
/// provider pause set by one busy reply delays every job's next send to that host
/// (REQ-SAG-0012).
///
/// # Lingering threads
///
/// Dropping a result iterator before it ends (abandon) joins no thread. A worker blocked
/// in a send exits when the send returns, at most the transport's request deadline
/// (`Timeouts::request`, or the request's own deadline) later. Worst case, the threads
/// still alive are `workers` per abandoned run plus one feeder per run whose caller
/// iterator is blocked in `next()`, for every run abandoned within one request deadline.
/// `runner.stream.end` reports `live_threads`: the runner threads (of every run of this
/// runner) alive at that moment.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
///
/// use sc_agent_http::testing::ScriptedTransport;
/// use sc_agent_http::{Endpoint, HttpClient, HttpRequest, HttpResponse, LogContext, Method, ResendPolicy};
/// use sc_agent_log::testing::MemoryLog;
/// use sc_agent_log::{Repo, SessionIdentity};
/// use sc_agent_types::CancelToken;
/// use sc_decision_runner::{DecisionRunner, RunnerConfig};
/// use sc_decision_schema::{Backend, Decision, DecisionError, DecisionRequest, SystemOneRequest, SystemOneResponse};
///
/// #[derive(Debug)]
/// struct Echo;
///
/// impl Backend for Echo {
///     fn encode(&self, request: &DecisionRequest) -> Result<HttpRequest, DecisionError> {
///         let endpoint = Endpoint::parse("https://api.example.test/v1/systemone")?;
///         Ok(HttpRequest::new(Method::Post, endpoint, request.body().to_json()?.into_bytes()))
///     }
///
///     fn decode(&self, response: HttpResponse, _: &DecisionRequest, _: &LogContext) -> Result<SystemOneResponse, DecisionError> {
///         SystemOneResponse::from_json(response.status(), response.body())
///     }
/// }
///
/// let body = br#"{"model":"m","answers":{"q":{"type":"noul","noul":0.5}},"usage":{"input_tokens":1,"output_tokens":1}}"#;
/// let transport = ScriptedTransport::responses(vec![HttpResponse::new(200, vec![], body.to_vec())]);
/// let log = MemoryLog::new()?;
/// let http = HttpClient::new(Arc::new(transport), ResendPolicy::default(), log.general());
/// let identity = SessionIdentity::new(Repo::local("host", "folder")?, "demo", "doc-test")?;
/// let runner = DecisionRunner::new(Arc::new(Echo), http, log.session(identity), log.general(), RunnerConfig::default());
///
/// let request = DecisionRequest::try_from(SystemOneRequest::from_json(
///     r#"{"model":"m","state":"s","questions":{"q":{"type":"noul"}}}"#,
/// )?)?;
/// let result = runner.decide(request, &CancelToken::new());
/// assert_eq!(result.position, 0);
/// assert!(result.outcome.is_ok());
/// runner.flush_session_log()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct DecisionRunner {
    shared: Arc<Shared>,
    config: RunnerConfig,
}

/// Everything a worker, the feeder and the result iterator share.
struct Shared {
    backend: Arc<dyn Backend>,
    http: HttpClient,
    session: SessionLogger,
    general: GeneralLogger,
    /// `Instant::now`, or a test clock; passed to `may_start`, `may_resend` and `send_once`.
    now: Clock,
    /// Runner threads (workers and feeders of every run) alive now.
    live_threads: Arc<AtomicUsize>,
}

impl DecisionRunner {
    /// A runner of `backend` sending through `http`, one row per result to `session`,
    /// runner events to `general`.
    ///
    /// `config` is valid by construction ([`RunnerConfig::new`]: 1..=64 workers, capacity
    /// 4 x workers), so there is nothing left to check here.
    #[must_use]
    pub fn new(
        backend: Arc<dyn Backend>,
        http: HttpClient,
        session: SessionLogger,
        general: GeneralLogger,
        config: RunnerConfig,
    ) -> Self {
        Self::with_clock(
            backend,
            http,
            session,
            general,
            config,
            Arc::new(Instant::now),
        )
    }

    /// [`Self::new`] with an explicit clock (crate-private test seam).
    pub(crate) fn with_clock(
        backend: Arc<dyn Backend>,
        http: HttpClient,
        session: SessionLogger,
        general: GeneralLogger,
        config: RunnerConfig,
        now: Clock,
    ) -> Self {
        Self {
            shared: Arc::new(Shared {
                backend,
                http,
                session,
                general,
                now,
                live_threads: Arc::new(AtomicUsize::new(0)),
            }),
            config,
        }
    }

    /// Flushes the session log and reports rows refused or lost since the last report.
    ///
    /// # Errors
    ///
    /// [`RunnerError::SessionLog`] with `log.flush` when the flush fails, or
    /// `log.rows_lost` when any row was refused or lost.
    pub fn flush_session_log(&self) -> Result<(), RunnerError> {
        self.shared.session.flush().map_err(RunnerError::SessionLog)
    }

    /// Starts one run of `requests` with `workers` workers.
    ///
    /// The cancel hook is registered before any thread starts, so a token cancelled
    /// before or during the start sends nothing.
    fn run(&self, requests: Requests, cancel: &CancelToken, workers: NonZeroUsize) -> ResultStream {
        let shared = Arc::clone(&self.shared);
        let run_id = SessionId::new();
        let capacity = self.config.queue_capacity().get();
        shared.event(
            Mode::Blocking,
            Level::Info,
            "runner.stream.start",
            LogContext::NONE,
            [
                ("run_id", run_id.to_string().into()),
                ("workers", workers.get().into()),
                ("queue_capacity", capacity.into()),
            ],
        );
        let jobs = Arc::new(RunnerJobs::new(capacity, Arc::clone(&shared.now)));
        // Producers: every worker and the feeder.
        let results = Arc::new(Results::new(capacity, workers.get().saturating_add(1)));
        let hook = {
            let jobs = Arc::clone(&jobs);
            let results = Arc::clone(&results);
            // Only flips state and notifies: never blocks, never does I/O. Runs at once
            // when the token is already cancelled.
            cancel.on_cancel(Arc::new(move || {
                jobs.cancel();
                results.interrupt();
            }))
        };
        let feed = Arc::new(Feed::default());
        let work = Arc::new(RunnerWork {
            shared: Arc::clone(&shared),
            jobs: Arc::clone(&jobs),
            run_id,
        });
        let pool = match Pool::start(
            workers,
            Arc::clone(&jobs),
            Arc::clone(&results),
            work,
            Arc::clone(&shared.live_threads),
        ) {
            Ok(pool) => pool,
            Err(error) => {
                results.producer_done();
                return ResultStream::failed(shared, run_id, requests, worker_error(&error));
            }
        };
        // Hands the caller's iterator to the feeder thread. A slot, not a plain move, so
        // the iterator comes back when the spawn fails (the closure is then dropped unrun)
        // and those requests are still answered. Locked once on each side.
        let slot = Arc::new(Mutex::new(Some(requests)));
        let feeder = Feeder {
            shared: Arc::clone(&shared),
            jobs: Arc::clone(&jobs),
            results: Arc::clone(&results),
            feed: Arc::clone(&feed),
        };
        let feeder_slot = Arc::clone(&slot);
        shared.live_threads.fetch_add(1, Ordering::AcqRel);
        let spawned = thread::Builder::new()
            .name("sc-decision-feeder".to_owned())
            .spawn(move || {
                let requests = feeder_slot
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .take();
                feeder.feed(requests);
            });
        if let Err(error) = spawned {
            shared.live_threads.fetch_sub(1, Ordering::AcqRel);
            pool.abandon();
            results.producer_done();
            let requests = slot
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take()
                .unwrap_or_else(|| Box::new(iter::empty()));
            return ResultStream::failed(shared, run_id, requests, worker_error(&error));
        }
        ResultStream::Running(Box::new(Running {
            shared,
            run_id,
            jobs,
            results,
            feed,
            ready: VecDeque::new(),
            counts: Counts::default(),
            ended: false,
            pool,
            _hook: hook,
        }))
    }
}

impl fmt::Debug for DecisionRunner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DecisionRunner")
            .field("backend", &self.shared.backend)
            .field("http", &self.shared.http)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl Decision for DecisionRunner {
    /// One request, position 0, on one runner worker (never the caller's thread); waits
    /// for its one result. Cancel returns `decision.cancelled { sends }` at once, in
    /// flight or held off.
    fn decide(&self, request: DecisionRequest, cancel: &CancelToken) -> DecisionResult {
        let identity = Identity::new(SessionId::new(), 0, &request);
        let mut stream = self.run(Box::new(iter::once(request)), cancel, NonZeroUsize::MIN);
        let Some(result) = stream.next() else {
            // Not reached: every submitted request gets one result. Kept total.
            let outcome = Err(DecisionError::Shutdown {
                reason: "the runner ended without a result".to_owned(),
            });
            return finish(&self.shared, &identity, outcome, Mode::Blocking).map_or_else(
                || decision_result(&identity, Err(cancelled(0))),
                Finished::into_result,
            );
        };
        // Ends the run cleanly (one result was all there was).
        let _end = stream.next();
        result
    }

    /// Feeder plus `config.workers` workers; results in finish order, exactly one per
    /// request. Dropping the iterator abandons the run (see the crate docs); it never
    /// cancels `cancel`.
    fn decide_stream<'a>(
        &'a self,
        requests: Box<dyn Iterator<Item = DecisionRequest> + Send + 'static>,
        cancel: &CancelToken,
    ) -> Box<dyn Iterator<Item = DecisionResult> + Send + 'a> {
        Box::new(self.run(requests, cancel, self.config.workers()))
    }
}

impl Shared {
    /// Writes one runner event; a log error never changes a result (the logger counts it).
    fn event<const N: usize>(
        &self,
        mode: Mode,
        level: Level,
        action: &'static str,
        ctx: LogContext,
        fields: [(&str, Value); N],
    ) {
        let fields: Map<String, Value> = fields
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect();
        let _counted_by_the_logger = match mode {
            Mode::Blocking => self.general.write(level, LOG_TARGET, action, ctx, fields),
            Mode::NonBlocking => self
                .general
                .try_write(level, LOG_TARGET, action, ctx, fields),
        };
    }

    fn live_threads(&self) -> usize {
        self.live_threads.load(Ordering::Acquire)
    }

    /// `runner.stream.end` of run `run_id`.
    fn stream_end(&self, mode: Mode, run_id: SessionId, counts: Counts, abandoned: Option<usize>) {
        let base = [
            ("run_id", run_id.to_string().into()),
            ("results", counts.results.into()),
            ("failed", counts.failed.into()),
            ("live_threads", self.live_threads().into()),
        ];
        match abandoned {
            None => self.event(
                mode,
                Level::Info,
                "runner.stream.end",
                LogContext::NONE,
                base,
            ),
            Some(dropped) => {
                let [run, results, failed, live] = base;
                self.event(
                    mode,
                    Level::Info,
                    "runner.stream.end",
                    LogContext::NONE,
                    [
                        run,
                        results,
                        failed,
                        live,
                        ("abandoned", true.into()),
                        ("dropped", dropped.into()),
                    ],
                );
            }
        }
    }
}

/// `decision.shutdown` reason for a thread that could not start.
fn worker_error(error: &std::io::Error) -> String {
    format!("a runner thread could not start: {error}")
}

/// `decision.cancelled` after `sends` sends.
fn cancelled(sends: u32) -> DecisionError {
    DecisionError::Cancelled { sends }
}

/// The one place a result is built.
fn decision_result(
    identity: &Identity,
    outcome: Result<SystemOneResponse, DecisionError>,
) -> DecisionResult {
    DecisionResult::new(
        identity.session_id,
        identity.position,
        identity.digests.clone(),
        outcome,
    )
}

/// Finishes the request of `identity` with `outcome`: wins its claim, writes its row,
/// builds its result. `None` when the claim was already won (the result was given).
///
/// The only place that pairs a [`Finish`](crate::row::Finish) and a
/// [`RowWritten`](crate::row::RowWritten) with a result: the row and the result are built
/// from the same `identity` and `outcome` here, so they describe the same request.
fn finish(
    shared: &Shared,
    identity: &Identity,
    outcome: Result<SystemOneResponse, DecisionError>,
    mode: Mode,
) -> Option<Finished> {
    let proof = identity.claim.win()?;
    let row = Row {
        position: identity.position,
        model: &identity.model,
        tally: identity.progress.tally(),
        digests: &identity.digests,
        outcome: &outcome,
    };
    let written = write_row(
        &shared.session,
        &shared.general,
        proof,
        identity.session_id,
        &row,
        mode,
    );
    Some(Finished::new(written, decision_result(identity, outcome)))
}

/// Finishes `identity` as cancelled after the sends it made (non-blocking row write).
fn finish_cancelled(shared: &Shared, identity: &Identity) -> Option<Finished> {
    let sends = identity.progress.tally().sends;
    finish(shared, identity, Err(cancelled(sends)), Mode::NonBlocking)
}

/// Who a request is, and how far it got: kept with the job from the feeder to its
/// result. Clones share the claim and the progress.
#[derive(Debug, Clone)]
struct Identity {
    session_id: SessionId,
    position: u64,
    model: String,
    digests: OrderedMap<String, QuestionDigest>,
    claim: Arc<Claim>,
    /// Sends and schema resends so far; updated when each send starts, so a panic,
    /// cancel or orphan path reads the sends really made.
    progress: Arc<Progress>,
}

impl Identity {
    fn new(session_id: SessionId, position: u64, request: &DecisionRequest) -> Self {
        Self {
            session_id,
            position,
            model: request.body().model.clone(),
            digests: request.digests().clone(),
            claim: Arc::new(Claim::new()),
            progress: Arc::new(Progress::default()),
        }
    }

    fn log_context(&self) -> LogContext {
        LogContext::request(self.session_id, Some(self.position))
    }
}

/// A queued request.
#[derive(Debug)]
enum Job {
    /// Not encoded yet.
    Fresh(Box<Fresh>),
    /// Encoded once; sent or held.
    Prepared(Box<Prepared>),
}

impl Job {
    fn prepared(core: Core, state: SendState) -> Self {
        Self::Prepared(Box::new(Prepared { core, state }))
    }

    /// `LogContext::request(session_id, Some(position))`.
    #[cfg(test)]
    fn log_context(&self) -> LogContext {
        self.identity().log_context()
    }

    fn identity(&self) -> &Identity {
        match self {
            Self::Fresh(fresh) => &fresh.identity,
            Self::Prepared(prepared) => &prepared.core.identity,
        }
    }
}

/// A request before its one encode.
#[derive(Debug)]
struct Fresh {
    identity: Identity,
    request: DecisionRequest,
}

impl Fresh {
    /// Encodes the request once.
    ///
    /// # Errors
    ///
    /// The job back with the backend's encode error; the job finishes with it.
    fn prepare(
        self: Box<Self>,
        backend: &dyn Backend,
    ) -> Result<Prepared, (Box<Self>, DecisionError)> {
        match backend.encode(&self.request) {
            Ok(http) => {
                let Self { identity, request } = *self;
                Ok(Prepared {
                    core: Core {
                        identity,
                        request,
                        http,
                    },
                    state: SendState::Start {
                        attempts: Attempts::new(),
                        mismatch: None,
                    },
                })
            }
            Err(error) => Err((self, error)),
        }
    }
}

/// An encoded request and where its resend state is.
#[derive(Debug)]
struct Prepared {
    core: Core,
    state: SendState,
}

/// The parts of a prepared request that do not change between sends.
#[derive(Debug)]
struct Core {
    identity: Identity,
    request: DecisionRequest,
    /// Encoded once: every send (busy, timeout or schema resend) sends these bytes.
    http: HttpRequest,
}

/// Where the request's resend state lives between sends (the `SendPermit` carries it
/// during a send). One per request: busy, timeout and schema resends share it.
#[derive(Debug)]
enum SendState {
    /// First send, or a resend after a final reply (`mismatch` = the last schema mismatch).
    Start {
        attempts: Attempts,
        mismatch: Option<DecisionError>,
    },
    /// Held after `Next::ResendAt`.
    Resend(Pending),
}

/// The work of a run's pool.
struct RunnerWork {
    shared: Arc<Shared>,
    jobs: Arc<RunnerJobs>,
    run_id: SessionId,
}

impl Work for RunnerWork {
    type Job = Job;
    type Answer = Finished;
    type InFlight = Identity;
    type Receipt = Identity;

    fn receipt(&self, job: &Job) -> Identity {
        job.identity().clone()
    }

    fn run(&self, job: Job) -> Step<Job, Finished> {
        let prepared = match job {
            Job::Fresh(fresh) => match fresh.prepare(&*self.shared.backend) {
                Ok(prepared) => prepared,
                Err((fresh, error)) => return self.finish(&fresh.identity, Err(error)),
            },
            Job::Prepared(prepared) => *prepared,
        };
        self.step(prepared)
    }

    fn skipped(&self, receipt: Identity, why: Skipped) -> Option<Finished> {
        match why {
            Skipped::Cancelled => finish_cancelled(&self.shared, &receipt),
            Skipped::Panicked(text) => finish(
                &self.shared,
                &receipt,
                Err(DecisionError::Shutdown {
                    reason: format!("a runner worker caught a panic: {text}"),
                }),
                Mode::Blocking,
            ),
        }
    }

    fn stopped(&self, worker: usize, why: Stopped) {
        let mode = match why {
            Stopped::InputEnded => Mode::Blocking,
            Stopped::Cancelled => Mode::NonBlocking,
        };
        self.shared.event(
            mode,
            Level::Info,
            "runner.worker.stopped",
            LogContext::NONE,
            [
                ("run_id", self.run_id.to_string().into()),
                ("worker", worker.into()),
                ("reason", why.as_str().into()),
            ],
        );
    }
}

impl RunnerWork {
    /// Finishes on the worker path (blocking row write).
    fn finish(
        &self,
        identity: &Identity,
        outcome: Result<SystemOneResponse, DecisionError>,
    ) -> Step<Job, Finished> {
        to_step(finish(&self.shared, identity, outcome, Mode::Blocking))
    }

    /// Asks the client for leave to send, then sends, holds or finishes.
    fn step(&self, prepared: Prepared) -> Step<Job, Finished> {
        let Prepared { core, state } = prepared;
        let now = (self.shared.now)();
        match state {
            SendState::Start { attempts, mismatch } => {
                let sent = attempts.sends();
                match self.shared.http.may_start(&core.http, attempts, now) {
                    // Sends or budget used up: only after a final reply, so the result is
                    // the mismatch it kept (REQ-SAG-0901), never http.provider_busy.
                    Err(reason) => {
                        let error = mismatch.unwrap_or_else(|| DecisionError::Shutdown {
                            reason: format!(
                                "the HTTP client refused the first send ({})",
                                reason.as_str()
                            ),
                        });
                        self.finish(&core.identity, Err(error))
                    }
                    Ok(Start::Wait { at, state }) => Step::HoldOff {
                        until: at,
                        job: Job::prepared(
                            core,
                            SendState::Start {
                                attempts: state,
                                mismatch,
                            },
                        ),
                    },
                    Ok(Start::Go(permit)) => self.send(core, permit, sent, mismatch.is_some()),
                }
            }
            SendState::Resend(pending) => {
                let sent = pending.attempts().sends();
                match self.shared.http.may_resend(&core.http, pending, now) {
                    Err(error) => self.finish(&core.identity, Err(DecisionError::Http(error))),
                    Ok(Start::Wait { at, state }) => Step::HoldOff {
                        until: at,
                        job: Job::prepared(core, SendState::Resend(state)),
                    },
                    Ok(Start::Go(permit)) => self.send(core, permit, sent, false),
                }
            }
        }
    }

    /// One send of an admitted request, registered in flight so cancel can answer it.
    /// `schema_resend`: this send follows a schema mismatch.
    fn send(
        &self,
        core: Core,
        permit: SendPermit,
        sent: u32,
        schema_resend: bool,
    ) -> Step<Job, Finished> {
        let Ok(key) = self.jobs.start_send(core.identity.clone()) else {
            return to_step(finish_cancelled(&self.shared, &core.identity));
        };
        let sends = sent.saturating_add(1);
        core.identity.progress.send_started(sends, schema_resend);
        let ctx = core.identity.log_context();
        let next = {
            let _in_flight = InFlightGuard {
                jobs: &self.jobs,
                key,
            };
            self.shared
                .http
                .send_once(&core.http, permit, ctx, &*self.shared.now)
        };
        if self.jobs.is_cancelled() {
            // Cancelled during the send: the late reply is discarded.
            return to_step(finish_cancelled(&self.shared, &core.identity));
        }
        match next {
            Err(error) => self.finish(&core.identity, Err(DecisionError::Http(error))),
            Ok(Next::ResendAt { at, pending }) => Step::HoldOff {
                until: at,
                job: Job::prepared(core, SendState::Resend(pending)),
            },
            Ok(Next::Done { response, attempts }) => {
                match self.shared.backend.decode(response, &core.request, &ctx) {
                    Err(error @ DecisionError::SchemaMismatch { .. }) => Step::HoldOff {
                        until: (self.shared.now)(),
                        job: Job::prepared(
                            core,
                            SendState::Start {
                                attempts,
                                mismatch: Some(error),
                            },
                        ),
                    },
                    outcome => self.finish(&core.identity, outcome),
                }
            }
        }
    }
}

/// An answer, or nothing when the claim was already won.
fn to_step(finished: Option<Finished>) -> Step<Job, Finished> {
    finished.map_or(Step::Discarded, Step::Done)
}

/// Removes a send's in-flight entry when the send returns, or unwinds.
struct InFlightGuard<'a> {
    jobs: &'a RunnerJobs,
    key: u64,
}

impl Drop for InFlightGuard<'_> {
    fn drop(&mut self) {
        self.jobs.end_send(self.key);
    }
}

/// How far the feeder got: requests answered or submitted, and whether it ended.
#[derive(Debug, Default)]
struct Feed {
    total: AtomicU64,
    done: AtomicBool,
}

impl Feed {
    fn count(&self) {
        self.total.fetch_add(1, Ordering::AcqRel);
    }

    /// `Some(total)` once the feeder ended.
    fn finished_total(&self) -> Option<u64> {
        self.done
            .load(Ordering::Acquire)
            .then(|| self.total.load(Ordering::Acquire))
    }
}

/// The feeder thread's state: pulls the caller's iterator into the job queue.
struct Feeder {
    shared: Arc<Shared>,
    jobs: Arc<RunnerJobs>,
    results: Arc<Results>,
    feed: Arc<Feed>,
}

impl Feeder {
    /// Submits every request with its position and a new session id.
    ///
    /// A submit refused by a cancel is answered `decision.cancelled { sends: 0 }`, and so
    /// is every later request, so each request gets one result. Once the result iterator
    /// is dropped (results discarded) the feeder stops pulling.
    fn feed(self, requests: Option<Requests>) {
        let _end = FeedEnd { feeder: &self };
        let Some(requests) = requests else {
            return;
        };
        for (position, request) in (0_u64..).zip(requests) {
            let identity = Identity::new(SessionId::new(), position, &request);
            match self
                .jobs
                .submit(Job::Fresh(Box::new(Fresh { identity, request })))
            {
                Ok(()) => self.feed.count(),
                Err(_) if self.results.is_discarded() => break,
                Err(job) => {
                    self.feed.count();
                    let identity = job.identity().clone();
                    drop(job);
                    if let Some(answer) = finish_cancelled(&self.shared, &identity)
                        && self.results.push(answer).is_err()
                    {
                        break;
                    }
                }
            }
        }
    }
}

/// Runs when the feeder ends, a panic in the caller's iterator included.
struct FeedEnd<'a> {
    feeder: &'a Feeder,
}

impl Drop for FeedEnd<'_> {
    fn drop(&mut self) {
        let feeder = self.feeder;
        feeder.feed.done.store(true, Ordering::Release);
        feeder.jobs.close();
        feeder.results.interrupt();
        feeder.results.producer_done();
        feeder.shared.live_threads.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Results yielded so far.
#[derive(Debug, Default, Clone, Copy)]
struct Counts {
    results: u64,
    failed: u64,
}

impl Counts {
    fn yielded(&mut self, result: DecisionResult) -> DecisionResult {
        self.results += 1;
        if result.outcome.is_err() {
            self.failed += 1;
        }
        result
    }
}

/// The result iterator of one run.
enum ResultStream {
    /// Workers and feeder started.
    Running(Box<Running>),
    /// No thread could start: every request is answered `decision.shutdown`.
    Failed(Failed),
}

impl ResultStream {
    fn failed(shared: Arc<Shared>, run_id: SessionId, requests: Requests, reason: String) -> Self {
        Self::Failed(Failed {
            shared,
            run_id,
            requests: (0_u64..).zip(requests),
            reason,
            counts: Counts::default(),
            ended: false,
        })
    }
}

impl Iterator for ResultStream {
    type Item = DecisionResult;

    fn next(&mut self) -> Option<DecisionResult> {
        match self {
            Self::Running(running) => running.next(),
            Self::Failed(failed) => failed.next(),
        }
    }
}

struct Running {
    shared: Arc<Shared>,
    run_id: SessionId,
    jobs: Arc<RunnerJobs>,
    results: Arc<Results>,
    feed: Arc<Feed>,
    /// Orphans and cancelled jobs answered and not yet yielded.
    ready: VecDeque<DecisionResult>,
    counts: Counts,
    ended: bool,
    /// Dropped after `Drop::drop`; never joined.
    pool: Pool<RunnerWork>,
    /// Lives as long as the run.
    _hook: sc_agent_types::CancelHook,
}

impl Running {
    fn next(&mut self) -> Option<DecisionResult> {
        if self.ended {
            return None;
        }
        loop {
            if let Some(result) = self.ready.pop_front() {
                return Some(self.counts.yielded(result));
            }
            // Read before the orphans, so an interrupt after this point wakes the pop.
            let seen = self.results.generation();
            self.answer_cancelled();
            if !self.ready.is_empty() {
                continue;
            }
            if self.feed.finished_total() == Some(self.counts.results) {
                self.end();
                return None;
            }
            match self.results.pop(seen) {
                Popped::Item(finished) => return Some(self.counts.yielded(finished.into_result())),
                Popped::Interrupted => {}
                Popped::Finished => {
                    self.end();
                    return None;
                }
            }
        }
    }

    /// After a cancel, answers every in-flight send cut off by it and every queued or
    /// held job, on this thread, now: no wait for a worker or the transport.
    fn answer_cancelled(&mut self) {
        let queued = self
            .jobs
            .take_cancelled()
            .into_iter()
            .map(|job| job.identity().clone());
        let identities: Vec<Identity> =
            self.jobs.take_orphans().into_iter().chain(queued).collect();
        for identity in identities {
            if let Some(answer) = finish_cancelled(&self.shared, &identity) {
                self.ready.push_back(answer.into_result());
            }
        }
    }

    fn end(&mut self) {
        self.ended = true;
        self.shared
            .stream_end(Mode::Blocking, self.run_id, self.counts, None);
    }
}

impl Drop for Running {
    /// Abandon: stop intake and cancel the job queue (never the caller's token), write the
    /// row of every orphan and every queued or held job here (`try_write`; results are
    /// discarded), join nothing. A worker blocked in a send lingers at most the
    /// transport's request deadline, then loses the claim and exits.
    fn drop(&mut self) {
        if self.ended {
            return;
        }
        // Discard first, so the feeder sees the consumer is gone and stops pulling.
        self.pool.abandon();
        self.answer_cancelled();
        let dropped = std::mem::take(&mut self.ready).len();
        self.shared
            .stream_end(Mode::NonBlocking, self.run_id, self.counts, Some(dropped));
    }
}

/// The result iterator when no thread could start.
struct Failed {
    shared: Arc<Shared>,
    run_id: SessionId,
    requests: iter::Zip<std::ops::RangeFrom<u64>, Requests>,
    reason: String,
    counts: Counts,
    ended: bool,
}

impl Failed {
    fn next(&mut self) -> Option<DecisionResult> {
        if self.ended {
            return None;
        }
        let Some((position, request)) = self.requests.next() else {
            self.ended = true;
            self.shared
                .stream_end(Mode::Blocking, self.run_id, self.counts, None);
            return None;
        };
        let identity = Identity::new(SessionId::new(), position, &request);
        let outcome = Err(DecisionError::Shutdown {
            reason: self.reason.clone(),
        });
        let result = finish(&self.shared, &identity, outcome, Mode::Blocking).map_or_else(
            || decision_result(&identity, Err(cancelled(0))),
            Finished::into_result,
        );
        Some(self.counts.yielded(result))
    }
}

#[cfg(test)]
mod tests;
