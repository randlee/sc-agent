<!-- Generated from bead agt-4p2.9 for file-based reviewers. The bead is the source of truth: edit the bead, then re-export. -->

# Sprint A-9: sc-decision-runner
- id: `a-9` (bead `agt-4p2.9`)
- branch: `sprint/a-9-decision-runner`
- depends on: a-2
- copy script: `docs/plans/phase-a/copy/a-9.sh`

## Description

Implements Decision for any Arc<dyn Backend> over an HttpClient: queue, workers, holdoff in the queue, cancellation via CancelToken hooks, schema-mismatch resend, one session row per result, results fastest first, failed requests returned while the stream continues. Records: REQ-SAG-0701, REQ-SAG-0601, REQ-SAG-0202, REQ-SAG-0901, ADR-SAG-0001.

## Design

```yaml
sprint:
  id: a-9
  closure_type: boundary
  target_boundary: sc-decision-runner
  owned_paths: [crates/sc-decision-runner/**]
  must_follow:
    - sprint: a-2
      artifact: sc_decision_schema::{Decision, Backend, DecisionRequest, DecisionResult, DecisionError} (DecisionRequest::new/digests)
      trigger: dev-push
    - sprint: a-5
      artifact: sc_agent_http::{HttpClient (may_start, may_resend, send_once), Start, SendPermit, Attempts, Pending, Next, LogContext (re-export), ResendPolicy, BusyReason, ProviderPause} + testing::{ScriptedTransport, BlockingTransport, ManualClock} (feature `testing`)
      trigger: dev-push
    - sprint: a-8
      artifact: sc_agent_log::{SessionLogger, GeneralLogger, Level, LogError} + testing::{MemoryLog, MemoryRecord} (feature `testing`)
      trigger: dev-push
    - sprint: a-4
      artifact: sc_agent_config::{AgentConfig, DEFAULT_WORKERS}
      trigger: dev-push
  requirements: [REQ-SAG-0701, REQ-SAG-0601, REQ-SAG-0602, REQ-SAG-0901, REQ-SAG-0202, REQ-SAG-0201, REQ-SAG-0008, REQ-SAG-0001, REQ-SAG-0002, REQ-SAG-0004, REQ-SAG-0007, REQ-SAG-0009, REQ-SAG-0010, REQ-SAG-0104, REQ-SAG-0012, NFR-SAG-0001]
  adrs: [ADR-SAG-0001, ADR-SAG-0002, ADR-SAG-0004]
  does_not_close:
    - "REQ-SAG-0701 end to end (a parallel batch through decide --batch and sc-agent-py, against real backends): a-10; a-9 closes it at the runner boundary with scripted transports"
    - "Budget-vs-timeout rule: closed in a-5 (A5-04, A5-06); a-9 only calls HttpClient::may_start / may_resend"
    - "Busy classification, Retry-After floor and jitter, the provider pause (REQ-SAG-0012 rule), max_sends and the 90 s budget themselves: a-5 acceptance; a-9 closes only that one HttpClient's pause covers every worker"
    - "REQ-SAG-0011 interrupt (SIGINT/SIGTERM, KeyboardInterrupt cancel the token): a-10 wires the signals; a-9 supplies the cancel path they use"
```

# Sprint A-9: sc-decision-runner (bead agt-4p2.9)

Governing: REQ-SAG-0701 (batches in parallel, workers default 8, fastest first, failures returned, cancel), REQ-SAG-0601 (holdoff waits in the queue, cancel ends it at once, no sleep), REQ-SAG-0602 (quota passed through), REQ-SAG-0901 (a reply that does not match the required schema is resent unchanged up to max_sends, then returned as decision.schema_mismatch), REQ-SAG-0202 (digest recorded with the response), ADR-SAG-0001 (one Decision trait over any Backend), ADR-SAG-0002 / NFR-SAG-0001 (one session row per session), ADR-SAG-0004 (top decision layer: depends on sc-decision-schema and the layers below; no backend crate; decisions only), REQ-SAG-0008. Also binding: REQ-SAG-0001, 0002, 0004, 0007, 0009, 0010.
Owner decisions: owner-decision-trait, owner-busy-resend, owner-architecture, owner-no-sleep, owner-lowlevel-no-tokio, owner-local-decision-crate, owner-error-contract, owner-workers-default.

Copy script: `docs/plans/phase-a/copy/a-9.sh` (dry run 2026-11-09, reconciliation: 8 copy steps, 4 files, 546 lines). Target: `crates/sc-decision-runner/`.

## Shape in one paragraph

`DecisionRunner` owns one `Arc<dyn Backend>` (A-2), one `HttpClient` (A-5: transport, `ResendPolicy`, provider pause, general logger), a `SessionLogger` and a `GeneralLogger` (A-8), and implements `Decision`. Its fields live in one `Arc<Shared>` so worker and feeder threads (`thread::Builder::spawn`, `'static`) own a clone; every worker sends through that one `HttpClient`, so a provider pause set by one busy reply (REQ-SAG-0012, A-5) delays every job's next send to that host — the runner sees it only as `Start::Wait { at, .. }` and holds the job. `decide_stream` takes an owned `'static` iterator (A-2 §3) and starts one feeder thread (pulls the caller's iterator into a bounded `JobQueue`) and `workers` worker threads (default 8, `AgentConfig.workers`). A job is `Fresh` until a worker prepares it (`Fresh::prepare` encodes once; digests come from `DecisionRequest::digests`, A-2) into `Prepared`; only a `Prepared` job can be sent or held. Before each send the worker asks the `HttpClient` for a start (A-5 §3): a `Start` state calls `may_start(&request, attempts, now)`, a `Resend` state calls `may_resend(&request, pending, now)`. `Start::Go(permit)` -> `send_once(&request, permit, ctx, &*now)` with `ctx = LogContext::request(session_id, Some(position))`; `Start::Wait { at, state }` (the provider pause or a holdoff not yet over) holds the job until `at` with `state` back in it and frees the worker. `may_start` Err (sends or budget used up — only possible on a resend after a Done) finishes the job with the `SchemaMismatch` it kept (REQ-SAG-0901), never `http.provider_busy`; `may_resend` Err finishes it with `DecisionError::Http(error)` unchanged. `Next::ResendAt { at, pending }` holds the job until `at` as `Resend(pending)`. `Next::Done { response, attempts }` goes to `backend.decode(response, &request, &ctx)`; `Err(DecisionError::SchemaMismatch)` puts the job back as ready in `Start { attempts, mismatch: Some(error) }` with the same encoded bytes. Every send of a request (busy, timeout and schema resends alike) counts against one `max_sends` and one 90 s budget; anything else finishes the job. Idle workers wait with `Condvar::wait_timeout` to the earliest holdoff end; a `CancelToken::on_cancel` hook (A-1) cancels the queue and wakes every waiter at once. Exactly one result and one row per request is enforced by a value: each job carries an `Arc<Claim>`; `Claim::win(&self) -> Option<Finish>` hands out the one non-Clone `Finish` token, the row writer consumes it and returns a non-Clone `RowWritten`, and the result push consumes that, so no path can write a row or result without winning, or twice. Cancel also ends a request whose send is in flight: the worker registers the job in the queue's in-flight list before `send_once`; `cancel()` moves every in-flight entry to an orphan list and interrupts the result queue; the result iterator (on the consumer's thread) wins each orphan's claim and yields `Cancelled { sends }` with its one row, so the result arrives without waiting for the transport. When the blocked send later returns, the worker's `win()` is `None`: it discards the reply and writes nothing. The hook itself only flips state and notifies; it never blocks or does I/O. The result queue holds at most `queue_capacity` results; a worker whose push finds it full blocks there (after its send, holding no HTTP slot) until the consumer reads, so a slow consumer throttles workers and nothing buffers without bound; held jobs count toward the job capacity, so a provider pause can also stall feeding (accepted). Results come back fastest first. `decide` runs the same machinery with one request and one worker (`run(Box::new(iter::once(request)), cancel, 1)`), so its send runs on a runner worker, not the caller's thread, and cancel is immediate there too. A session-log write failure never replaces the decision result: the row's Err is logged to the general log and counted by the `SessionLogger`, and `flush_session_log` reports it.

Drop of the result iterator (`abandon`, RSH-006): (1) stop intake — the `JobQueue` is cancelled, so the feeder's next `submit` fails and it ends; a feeder blocked inside the caller's iterator `next()` cannot be interrupted and ends when that call returns; (2) every queued and held job is taken as `Cancelled` by the workers, which write its one row (results are discarded); (3) the dropping thread wins every orphan's claim and writes its one `Cancelled` row; (4) no thread is joined — workers blocked in a send linger at most the transport's request deadline (A-5 `Timeouts.request`, a global deadline), then lose the claim and exit; (5) the caller's `CancelToken` is never cancelled. Requests the feeder never pulled get no row.

## 1. Copy list (source `sc-agent-ref@dce239a` -> `crates/sc-decision-runner/`)

| # | Source path:lines | Target | What | Required edits after copying |
|---|---|---|---|---|
| C1 | `crates/sc-jev-host/LICENSE` | `LICENSE` | MIT | none |
| C2 | `crates/sc-jev-host/src/stream/pool.rs:32-200` | `src/queue.rs` | `Skipped`, `BoundedQueue<T>` (Mutex + Condvars, `close`, `discard`, poison recovered), `PushError`, `Popped` | `BoundedQueue` stays as the result queue: drop `pushed`/`popped`, `PushError::Full` and the timed push/pop (always block; woken by `close`/`discard`). `Skipped` keeps `Cancelled` and `Panicked`. Add `JobQueue<J>` (N2). |
| C3 | `crates/sc-jev-host/src/stream/pool.rs:202-297` | `src/pool.rs` | `Counters`, `WorkerPool::start/submit/close_input/cancel/recv` | Rename `Pool`. `RunFn` becomes `Fn(J) -> Step<J, A>`; delete `LostFn`, `Counters.running`, `workers`, `queue_capacity`. `start` uses `thread::Builder::new().name("sc-decision-worker").spawn` and returns `Result<Self, std::io::Error>`. `cancel()` cancels the `JobQueue` only, never the caller's token. `recv()` = `results.pop()`. |
| C4 | `crates/sc-jev-host/src/stream/pool.rs:337-417` | `src/pool.rs` | `join`, `abandon`, `Drop`, `Worker`, `Exit` guard, `work` loop with `catch_unwind` | `Drop`/`abandon`: `jobs.cancel()` + `results.discard()`. `work`: `Step::HoldOff { until, job }` -> `jobs.hold(job, until)`; `Done(a)` -> push; `jobs.done()` after every run. `lost` is gone. `#[cfg(test)] mod tests;`. |
| T1 | `crates/sc-jev-host/src/stream/pool/tests.rs:1-65, 76-91, 99-125, 213-283` | `src/pool/tests.rs` | `Rendezvous`, `pool_of`, `next`, `drain`; tests: workers overlap (2 and 8), cancel skips queued and refuses new, a panicking job is answered | Delete the sleep-polling `wait_until`/`join` helpers; use a "started" `mpsc` signal and `pool.join()`. `next` uses blocking `recv` (failsafe via `mpsc::recv_timeout` on a helper thread). Port closures to `Step::Done`. |

Not copied: `stream/mod.rs` (JSONL wire format, exit codes, `serve_jsonl`, `Batch`: CLI concerns, A-10; its `abandon` sleeps); `Concurrency` (`MAX = 64` is in no record); `stream/log.rs` (replaced by `GeneralLogger::write`); introspection counters; pool tests at `tests.rs:66-75` (sleep) and `126-212, 284-365`.

## 2. Interfaces consumed (defined by their owning sprints; not restated here)

- A-1 §3: `CancelToken` (`is_cancelled`, `on_cancel` -> `CancelHook`), `SessionId::new`, `ErrorId` (`cause_detail()`, provided `recovery()`), `ErrorDetail` (with optional `recovery`).
- A-2 §3 and §5: `Decision`, `Backend`, `DecisionRequest` (`new`, `body()`, `digests()`), `DecisionResult`, `DecisionError`.
- A-4 §3: `AgentConfig { workers, .. }`, `DEFAULT_WORKERS`.
- A-5 §3 and §4: `HttpClient::{send_once, policy}`, `HttpClient::{may_start, may_resend}`, `Start<S>`, `SendPermit`, `Attempts`, `Pending`, `ProviderPause`, `LogContext` (sc-agent-log's, re-exported), `Next`, `ResendPolicy`, `BusyReason`, `HttpRequest`, `HttpResponse`; `testing::{ScriptedTransport, BlockingTransport}` (dev).
- A-8 §2 and §3: `SessionLogger::{write, flush}`, `GeneralLogger::write`, `Level`, `LogError`, `SessionIdentity`; `GeneralLogger::{write, try_write}`, `SessionLogger::try_write`; `testing::{MemoryLog, MemoryRecord}` (dev): `MemoryLog::new() -> Result<MemoryLog, LogError>`, `general(&self) -> GeneralLogger`, `session(&self, identity: SessionIdentity) -> SessionLogger`, `records(&self) -> Result<Vec<MemoryRecord>, LogError>`; `MemoryRecord { level, target, action, fields }`.

Busy classification, quota, `Retry-After`, the schedule with jitter, max sends and the 90 s budget are A-5's. The runner only waits and requeues; it computes no holdoff.

## 3. Exact public Rust surface (bodies elided)

```rust
// src/lib.rs
#![forbid(unsafe_code)]
mod config; mod error; mod pool; mod queue; mod row; mod runner;
pub use config::RunnerConfig;
pub use error::RunnerError;
pub use runner::DecisionRunner;

// src/config.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunnerConfig {
    /// Worker threads (REQ-SAG-0701; AgentConfig.workers, default 8).
    pub workers: NonZeroUsize,
    /// Most jobs in the runner at once (queued + held + running); the feeder blocks past it. 4 x workers, a constant, not a config key (assistant decision).
    pub queue_capacity: NonZeroUsize,
}
impl RunnerConfig {
    /// workers from AgentConfig.workers; queue_capacity = workers * 4.
    pub fn from_config(config: &sc_agent_config::AgentConfig) -> Self;
}
impl Default for RunnerConfig { /* workers sc_agent_config::DEFAULT_WORKERS (8), queue_capacity 32 */ }

// src/runner.rs
pub struct DecisionRunner { shared: Arc<Shared>, config: RunnerConfig }
struct Shared {
    backend: Arc<dyn Backend>,
    http: HttpClient,
    session: SessionLogger,
    general: GeneralLogger,
    now: Clock,          // Instant::now; crate-private test seam (src/runner/tests.rs uses sc_agent_http::testing::ManualClock), passed to may_start/may_resend/send_once
}
type Clock = Arc<dyn Fn() -> Instant + Send + Sync>;
impl DecisionRunner {
    pub fn new(backend: Arc<dyn Backend>, http: HttpClient, session: SessionLogger, general: GeneralLogger, config: RunnerConfig) -> Self;
    /// Flushes the session log; RunnerError::SessionLog(log.rows_lost | log.flush) when any row was refused or lost.
    pub fn flush_session_log(&self) -> Result<(), RunnerError>;
}
impl Decision for DecisionRunner {
    /// One request, position 0, on one runner worker (never the caller's thread); the caller waits
    /// for its one result; cancel returns Cancelled { sends } at once, in flight or held off.
    fn decide(&self, request: DecisionRequest, cancel: &CancelToken) -> DecisionResult;
    /// Feeder + config.workers workers; results in finish order; exactly one result per request.
    fn decide_stream<'a>(&'a self, requests: Box<dyn Iterator<Item = DecisionRequest> + Send + 'static>, cancel: &CancelToken)
        -> Box<dyn Iterator<Item = DecisionResult> + Send + 'a>;
}

// crate-private
pub(crate) enum Step<J, A> { Done(A), HoldOff { until: Instant, job: J } }   // a schema resend is HoldOff { until: now }
pub(crate) struct JobQueue<J> { state: Mutex<JobState<J>>, changed: Condvar, capacity: usize }
struct JobState<J> { ready: VecDeque<J>, held: BinaryHeap<Held<J>>, running: usize, in_flight: Vec<InFlight>, orphans: Vec<InFlight>, closed: bool, cancelled: bool, seq: u64 }
struct InFlight { key: u64, session_id: SessionId, position: u64, model: String, digests: OrderedMap<String, QuestionDigest>, sends: u32 /* sends including this one, taken from SendState before the permit */, claim: Arc<Claim> }
struct Claim(AtomicBool);
impl Claim { fn win(&self) -> Option<Finish>; }      // compare_exchange(false, true): Some exactly once
/// Proof that this caller alone finishes the request. Not Clone, not Copy; only Claim::win makes one.
struct Finish(PhantomData<Claim>);
/// Proof that the one row was written (or its failure logged). Not Clone, not Copy.
struct RowWritten(PhantomData<Finish>);
fn write_row(session: &SessionLogger, general: &GeneralLogger, proof: Finish, row: Row) -> RowWritten;   // worker path: write; cancel/orphan/abandon paths: try_write (never blocks)
// result push: BoundedQueue<DecisionResult>::push(&self, proof: RowWritten, result: DecisionResult) (blocks while full)
struct Held<J> { until: Instant, seq: u64, job: J }       // Ord: earliest `until`, then `seq`
pub(crate) enum Taken<J> { Run(J), Cancelled(J), Finished }
impl<J> JobQueue<J> {
    fn submit(&self, job: J) -> Result<(), J>;  // blocks while ready+held+running >= capacity; Err(job) once closed or cancelled
    fn hold(&self, job: J, until: Instant);     // never blocks
    fn take(&self) -> Taken<J>;                 // ready first; due held jobs move to ready; else wait_timeout(earliest - now) or wait()
    fn done(&self);                             // running -= 1; notify_all
    fn close(&self);                            // input ended
    fn cancel(&self);                           // take() now yields Cancelled(job) for every ready and held job; in_flight -> orphans; notify_all
    fn start_send(&self, entry: InFlight);      // before send_once
    fn end_send(&self, key: u64);               // after send_once returns (removes from in_flight or orphans)
    fn take_orphans(&self) -> Vec<InFlight>;    // result iterator: in-flight jobs cut off by cancel
}
enum Job { Fresh(Fresh), Prepared(Prepared) }
struct Fresh { session_id: SessionId, position: u64, request: DecisionRequest, claim: Arc<Claim> }
struct Prepared {
    session_id: SessionId, position: u64, request: DecisionRequest,   // digests: request.digests()
    http: HttpRequest,                            // encoded once; every send (busy or schema resend) sends the same bytes
    state: SendState,                             // one per request: busy, timeout and schema resends share it; row "sends" from it
    schema_resends: u32,
    claim: Arc<Claim>,
}
impl Fresh { fn prepare(self, backend: &dyn Backend) -> Result<Prepared, (Fresh, DecisionError)>; }   // Err: encode failed; the job finishes with it
/// Where the request's resend state lives between sends (the SendPermit carries it during a send).
enum SendState {
    Start { attempts: Attempts, mismatch: Option<DecisionError> },  // first send, or resend after Done (mismatch = the last SchemaMismatch)
    Resend(Pending),                                                // after Next::ResendAt
}
impl Job { fn log_context(&self) -> LogContext; }   // LogContext::request(session_id, Some(position))
```

Session row (`src/row.rs`, one per result: ok, failed and cancelled; `SessionLogger` adds `session_id`, `repo`, `project`, `agent_role`):

```text
{ "position": <u64>, "model": <request.body().model>, "sends": <u32>, "schema_resends": <u32>,
  "outcome": "ok" | "error",
  "digests": { "<question id>": "<sc-sha hex>", ... },
  "response": <SystemOneResponse, unrounded>,           // ok
  "error": <ErrorDetail of the DecisionError> }          // error: id, message and every lower id and message
```

General log (`GeneralLogger::write`, target `sc_decision_runner`): `runner.stream.start` (workers), `runner.stream.end` (results, failed), `runner.worker.stopped` (reason), `runner.session_log_failed` (ctx = the job's `LogContext`; error detail). Stream-level events pass `LogContext { session_id: None, position: None }`. Every http.* event of a job's sends carries its `LogContext` through `send_once`, and `backend.decode(response, &request, &ctx)` gets the same one. Cancel, orphan and abandon paths use `try_write` only. No request or response content.

## 4. Errors

Results carry `DecisionError` (A-2), with the runner-level failures in it: `Cancelled { sends }` for every request not finished when the token is cancelled (queued, held off, refused at submit, or in flight — returned at once, the late reply discarded), `Shutdown { reason }` when no worker could start or a worker stopped (a caught panic's payload text kept). `Http`, `SchemaMismatch` (after `max_sends` mismatching replies), `Backend` and the decode errors pass through unchanged.

The crate's own union covers its one fallible method:

```rust
#[derive(Debug)]
#[non_exhaustive]
pub enum RunnerError {
    /// The session log could not be flushed or lost rows (LogError kept unchanged: log.flush, log.rows_lost).
    SessionLog(LogError),                       // "runner.session_log"
}
impl ErrorId for RunnerError {}                // cause_detail() = LogError detail; source() = Some(inner); Display = `id: message`
```

Lints: workspace `panic`/`unwrap_used`/`expect_used`/`indexing_slicing` deny (A-1); `catch_unwind` around backend and transport calls is the only panic handling.

## 5. New code (not in the copy)

- N1 `Cargo.toml`, `README.md`, `src/lib.rs`, `src/config.rs`, `src/error.rs`.
- N2 `JobQueue<J>` in `src/queue.rs` (~110 lines), incl. the in-flight/orphan lists; `BoundedQueue::interrupt()` (notify_all without closing) added on top of C2 so the result iterator wakes for orphans; the result `BoundedQueue` is built with capacity `queue_capacity` and its `push` takes `RowWritten`.
- N3 `src/runner.rs` (~200 lines): `DecisionRunner`, `run(requests: Box<dyn Iterator<Item = DecisionRequest> + Send + 'static>, cancel, workers)` shared by `decide` (workers 1, `iter::once`) and `decide_stream`; the step fn (`Fresh::prepare` -> `Start`: `http.may_start(&req, attempts, now)` / `Resend`: `http.may_resend(&req, pending, now)` -> `Go(permit)`: `start_send` -> `send_once(&req, permit, ctx, &*now)` -> `end_send`; `Wait { at, state }`: hold until `at`; `may_start` Err: finish with the kept mismatch; `may_resend` Err: finish `Http`; `win()` None -> discard; `ResendAt { at, pending }` hold / `Done { response, attempts }` -> `decode(response, &req, &ctx)` -> `SchemaMismatch` -> ready as `Start { attempts, mismatch }` / other: finish), the skipped fn (`Cancelled { sends }`, sends read from the job's `SendState`: `attempts.sends()` for `Start { attempts, .. }`, `pending.attempts().sends()` for `Resend(pending)`), the feeder thread (owns the `'static` iterator), the `on_cancel` hook calling `jobs.cancel()` + `results.interrupt()` (the `CancelHook` lives as long as the run), the result iterator (drains `take_orphans()` before popping; Drop = `abandon` as stated in the shape section, never cancels the caller's token).
- N4 `src/row.rs` (~30 lines).
- N5 `tests/support/scripted_backend.rs` (~50 lines, private to a-9's tests; `mod support;` in each test file): `ScriptedBackend { encode: EncodeFn, decode: DecodeFn, encodes: AtomicU32, decodes: AtomicU32 }` implementing `sc_decision_schema::Backend`; `new()` (encode: POST https://scripted.test/v1/systemone with `request.body().to_json()`; decode: `SystemOneResponse::from_json`), `with_encode(Fn(&DecisionRequest) -> Result<HttpRequest, DecisionError>)`, `with_decode(Fn(u32 /*0-based decode index*/, HttpResponse, &DecisionRequest, &LogContext) -> Result<SystemOneResponse, DecisionError>)`, `encodes()`, `decodes()`.
- Config keys used (A-4): `workers`, `max_sends` (through the caller's `ResendPolicy` in `HttpClient`). No new keys.

## 6. Tests (in-process; `ScriptedBackend` (a-9's own, `tests/support/scripted_backend.rs`), `ScriptedTransport` / `BlockingTransport` (A-5) and `MemoryLog` (A-8) as dev-dependencies with feature `testing`; delays wait on a Condvar, never sleep). Each test names the acceptance line it proves.

| Test | Acceptance line |
|---|---|
| `forty_requests_parallel` (Rendezvous(8) in the transport) | REQ-SAG-0701 AC1 |
| `one_worker_is_serial` | REQ-SAG-0701 AC2 |
| `failed_request_returned_rest_continue`, `fastest_first` | REQ-SAG-0701 AC3 |
| `cancel_running_batch` (token created in the test crate) | REQ-SAG-0701 AC4, REQ-SAG-0104 AC1 |
| `cancel_in_flight_send` (BlockingTransport; cancel; the Cancelled result is received on an mpsc channel (`recv_timeout(2 s)` failsafe) while the transport is still blocked; then release; no second result or row) — stream and `decide` | Cancel in flight, REQ-SAG-0104 AC1 |
| `decide_runs_on_a_worker` (transport records the thread name `sc-decision-worker`) | Cancel in flight |
| `static_iterator_accepted` (compile test: `Box::new(vec.into_iter())` passed to `decide_stream` through `&dyn Decision`) | decide_stream signature |
| `busy_four_then_ok`, `busy_every_time_stops_at_max_sends`, `same_bytes_each_send` | REQ-SAG-0601 resend |
| `holdoff_frees_the_worker` | REQ-SAG-0601 resend (wait in the queue) |
| `cancel_ends_holdoff_at_once`, `decide_cancel_during_holdoff` | REQ-SAG-0601 AC4 |
| `busy_quota_network_passed_through` | REQ-SAG-0601/0602 |
| `schema_mismatch_resent_once`, `cancel_ends_schema_resends` | REQ-SAG-0901 AC2 |
| `schema_mismatch_after_max_sends`, `busy_twice_then_mismatch_stops_at_five` | REQ-SAG-0901 AC3 |
| `budget_runs_out_after_mismatch` (ManualClock advanced 91 s after the first send; `may_start` -> Err(Budget)) | REQ-SAG-0901 budget |
| `one_row_per_result` (50 requests incl. failed, cancelled and cancelled-in-flight) | NFR-SAG-0001 / ADR-SAG-0002 |
| `three_questions_three_digests` | REQ-SAG-0202 |
| `two_backends_one_runner` | ADR-SAG-0001 |
| `lower_errors_reach_the_top`, `panic_in_backend_is_a_result`, `session_log_failure_keeps_result` | REQ-SAG-0002 |
| `dropping_the_stream_does_not_cancel_the_token` | Token ownership |
| `drop_mid_batch_one_row_per_started_job` (20 requests, BlockingTransport, drop after 3 results, release; MemoryLog rows: one per job the feeder submitted, no duplicate session_id) | Abandon |
| `slow_consumer_blocks_workers` (workers 2, queue_capacity 8, consumer reads nothing: transport calls stop at 8 + 2 — the next call does not arrive within an `mpsc::recv_timeout(500 ms)`; one read lets the next send through) | Result queue bound |
| `provider_pause_holds_other_jobs` (workers 2; A gets 529 `Retry-After: 1`; B's next send reaches the transport no earlier than A's pause end) | REQ-SAG-0012 (runner side) |
| `busy_records_carry_session` (two concurrent requests busy once; MemoryLog http.busy records carry the two different session_ids and positions) | LogContext |
| `runner_general_records` (`MemoryLog::new()?`; runner built with `log.general()` and `log.session(identity)`; `log.records()?` filtered on `target == "sc_decision_runner"` and `action`) | ANC-007 |
| ported pool tests (T1) | pool mechanics (no acceptance line; kept from the copy) |

## 6a. Required validation

- `cargo build -p sc-decision-runner`
- `cargo clippy -p sc-decision-runner --all-targets -- -D warnings` with the workspace lints (a-1's `[workspace.lints]`, clippy.toml disallowed-methods incl. sleep and `std::thread::spawn`)
- `cargo test -p sc-decision-runner`
- `cargo tree -p sc-decision-runner -e normal`: no tokio; no sc-decision-systemone / sc-decision-local / sc-decision-llm / sc-llm-provider; direct sc-agent dependencies as in AC ADR-SAG-0004 (`--depth 1`)
- `rg -n 'sleep\(|thread::spawn' crates/sc-decision-runner/src` finds nothing outside tests

## 7. Dependencies

- `sc-agent-types` (CancelToken, SessionId, ErrorId), `sc-agent-config` (AgentConfig, DEFAULT_WORKERS), `sc-decision-schema` (Decision, Backend, DecisionRequest/Result/Error, DecisionRequest::digests), `sc-agent-http` (HttpClient, HttpRequest, Start, SendPermit, Attempts, Pending, Next, LogContext, BusyReason), `sc-agent-log` (SessionLogger, GeneralLogger, Level, LogError) — all path. No sc-agent-secrets (keys never reach the runner; they live in the HttpRequest built by the backend).
- dev-dependencies: `sc-agent-http`, `sc-agent-log`, each with `features = ["testing"]`.
- `serde_json = "1"` (row payload).
- No backend crate (backends arrive as `Arc<dyn Backend>`), no sc-observability, no tokio, no thiserror.

## Acceptance criteria

- [ ] Copy first: the sprint's first commit is the unchanged output of `docs/plans/phase-a/copy/a-9.sh` (sc-agent-ref@dce239a), committed before any edit; every change needed to make that code build and pass comes in later commits (owner-copy-mechanically).
- [ ] REQ-SAG-0701 AC1: sc-decision-runner: DecisionRunner::decide_stream over 40 requests against a ScriptedTransport that answers each after a delay finishes in about one eighth of the serial time with the default 8 workers (8 transport calls observed in flight at once).
- [ ] REQ-SAG-0701 AC2: sc-decision-runner: with RunnerConfig.workers = 1 the same batch runs one request at a time; RunnerConfig::from_config takes workers from AgentConfig.workers, default 8.
- [ ] REQ-SAG-0701 AC3: sc-decision-runner: a batch with one failing request returns that error tagged with its session ULID and returns every other result; results come back in finish order, each with its session ULID and input position.
- [ ] REQ-SAG-0701 AC4: sc-decision-runner: cancelling a running decide_stream returns decision.cancelled for every unfinished request; one result per request, always.
- [ ] Cancel in flight: sc-decision-runner: with a BlockingTransport that blocks until the test releases it, cancelling delivers decision.cancelled { sends } while the transport is still blocked (received on a channel, 2 s failsafe bound) for decide_stream and for decide (decide's send runs on a `sc-decision-worker` thread, not the caller's); releasing the transport afterwards produces no further result and no second session row.
- [ ] REQ-SAG-0104 AC1: a CancelToken created in the test crate stops work inside sc-decision-runner: cancel_running_batch and cancel_in_flight_send end every unfinished request of DecisionRunner::decide_stream and decide with decision.cancelled.
- [ ] REQ-SAG-0012 (runner side): sc-decision-runner sends every job through its one HttpClient, so a provider pause set by one busy reply makes may_start/may_resend return Start::Wait for the other jobs to that host and the runner holds them (test: B's next send reaches the transport no earlier than the end of A's Retry-After pause); the pause rule itself is a-5's.
- [ ] Exactly once by construction: sc-decision-runner's row writer takes the non-Clone Finish token that only Claim::win returns, and the result push takes the RowWritten token the row writer returns; jobs are Fresh until prepared and only Prepared jobs are sent or held.
- [ ] Abandon: dropping sc-decision-runner's result iterator mid-batch stops intake, cancels the JobQueue (never the caller's token), joins no thread (a worker blocked in a send lingers at most the transport request deadline) and writes exactly one row per job the feeder submitted (test drop_mid_batch_one_row_per_started_job).
- [ ] Result queue bound: sc-decision-runner's result queue holds at most queue_capacity results and a worker blocks on a full queue (test slow_consumer_blocks_workers).
- [ ] LogContext: sc-decision-runner passes each job's LogContext::request(session_id, Some(position)) to HttpClient::send_once and Backend::decode; two concurrent busy requests' http.busy records carry their own session_ids; runner.session_log_failed carries session_id and position.
- [ ] decide_stream signature: sc-decision-runner implements decide_stream with `requests: Box<dyn Iterator<Item = DecisionRequest> + Send + 'static>` (A-2) and pulls it on its feeder thread; a compile test passes `Box::new(vec.into_iter())` through `&dyn Decision`.
- [ ] REQ-SAG-0601: sc-decision-runner resends a request whose HttpClient::send_once returns Next::ResendAt { at, pending } (HTTP 429, 502 or 529, classified by a-5), through HttpClient::may_resend, until max_sends (default 5, set in the HttpClient ResendPolicy) is reached, and sends the same encoded bytes each time (encode called once). Test: a ScriptedTransport answering 529 four times then 200 yields the answer; one answering 529 every time stops after max_sends sends. A held-off request waits in the JobQueue, not in a worker (with 1 worker a second request is answered during the first one's holdoff); the wait (ResendAt or Start::Wait) is a Condvar timed wait woken by a CancelToken::on_cancel hook; no sleep. Rows and general records on the cancel, orphan and abandon paths use try_write.
- [ ] REQ-SAG-0601 AC4: sc-decision-runner: cancelling a held-off request ends its wait at once and returns decision.cancelled (decide_stream and decide).
- [ ] REQ-SAG-0601/0602: sc-decision-runner resends only on Next::ResendAt (and a schema mismatch) and returns http.provider_busy / http.quota_exhausted / http.network unchanged inside decision.http.
- [ ] REQ-SAG-0901 AC2: sc-decision-runner: a test backend (tests/support ScriptedBackend) that first returns decision.schema_mismatch (a reply missing probabilities) and then a valid answer yields the valid answer after one resend (2 sends, schema_resends 1 in the row); resends stop at once on cancel.
- [ ] REQ-SAG-0901 AC3: sc-decision-runner: a backend that never matches yields decision.schema_mismatch, with what did not match, after max_sends sends in total (default 5; max_sends = 2 gives 2); a transport answering 529 twice and then a backend that never matches stops after 5 sends, not more.
- [ ] REQ-SAG-0901 budget: sc-decision-runner resends after a schema mismatch only through HttpClient::may_start(&request, attempts, now); when it refuses because the 90 s budget has run out (ManualClock 91 s after the first send) the result is decision.schema_mismatch, never decision.http/http.provider_busy.
- [ ] NFR-SAG-0001 / ADR-SAG-0002: sc-decision-runner writes exactly one session row per result (ok, failed, cancelled and cancelled in flight), each with its session ULID and the identity fields session_id, repo, project, agent_role; error rows carry the DecisionError ErrorDetail chain; "sends" is Attempts::sends().
- [ ] REQ-SAG-0202: sc-decision-runner: each ok row records one sc-sha question digest per question with the response (three questions -> three digests).
- [ ] ADR-SAG-0001: every backend sc-decision-runner drives implements sc_decision_schema::Backend (the sans-IO trait: encode a Jev request into an HTTP request, decode an HTTP response into a Jev response); DecisionRunner holds only an Arc<dyn sc_decision_schema::Backend> and implements the one Decision trait (no associated error) over it and an HttpClient. Test: the runner tests drive two different Backend implementations through the same DecisionRunner.
- [ ] ADR-SAG-0004: sc-decision-runner sits in the runner layer; its direct sc-agent crate dependencies are sc-agent-types, sc-agent-config, sc-agent-log, sc-agent-http and sc-decision-schema (no sc-agent-secrets), and it depends on no decision-backend crate, checked by `cargo tree -p sc-decision-runner -e normal --depth 1` and a full `cargo tree -p sc-decision-runner -e normal` with no sc-decision-systemone, sc-decision-local, sc-decision-llm or sc-llm-provider.
- [ ] REQ-SAG-0002: every error returned by a public sc-decision-runner function (decide, the decide_stream results, flush_session_log) carries the error id and message of each lower-layer error that caused it: HttpError inside decision.http, LogError inside runner.session_log, and the backend's own error. Test: inject a failure in the transport, in the session log and in backend decode, and assert that failure's id and message are in the returned DecisionError chain. RunnerError uses cause_detail() and Display `id: message`. Runner-level failures are decision.cancelled and decision.shutdown (a backend panic becomes decision.shutdown); a session-log write failure never replaces the decision result; no thread::spawn (Builder::spawn only); clippy with the workspace denies passes.
- [ ] Token ownership: dropping sc-decision-runner's result iterator never cancels the caller's CancelToken.
- [ ] REQ-SAG-0009/0010: `cargo tree -p sc-decision-runner` shows no tokio; a search of sc-decision-runner non-test source finds no sleep call.
- [ ] REQ-SAG-0010: sc-decision-runner waits out holdoffs and idle workers with Condvar timed waits woken by CancelToken::on_cancel, never a sleep function; enforced by the workspace clippy disallowed-methods rule.
- [ ] REQ-SAG-0201: sc-decision-runner defines no Rust type that maps to a field of the TypeSafe Jev API (POST /v1/systemone, GET /v1/models); every such type it uses (Request, Response, Backend, Decision, DecisionError) is imported from sc-decision-schema. Check: a search of crates/sc-decision-runner/src finds no serde type for an API field.
- [ ] ANC-007: sc-decision-runner writes runner.stream.start, runner.stream.end, runner.worker.stopped and runner.session_log_failed to the GeneralLogger passed to DecisionRunner::new; a test passes `sc_agent_log::testing::MemoryLog::general()` as that GeneralLogger, runs a stream with one failing request and finds those actions (target sc_decision_runner) in `MemoryLog::records()`.

## Notes

Closed 2026-11-09 by owner answers (search-for-truth step 7):
- Q1 queue capacity: 4 x workers, a constant (assistant).
- Schema resend limit: max_sends decoded replies, then decision.schema_mismatch (REQ-SAG-0901).
- Result correlation: session ULID and input position (REQ-SAG-0701).
No open questions.
Hardening round 1 (2026-11-09, decisions.md): PLAN-CRIT-001 'static request iterator, feeder kept; PLAN-CRIT-002 in-flight cancel via claim/orphans, decide on a worker (replaces "decide on the caller's thread with wait_until"); PLAN-CRIT-003 runner side only (may_start before a schema resend); budget-vs-timeout rule closed in a-5 (A5-04, A5-06). CRIT-M2 sc-agent-secrets dropped; CRIT-M4 Job.sends dropped.
Round 2 (2026-11-09, round2.md): REQ-QA-012 OPEN note removed; REQ-QA-007 REQ-SAG-0104 AC1 owned here; RBP-F003 Finish/RowWritten tokens; RBP-F009(1) Fresh/Prepared; RSH-006 abandon contract + test; RSH-011 result queue = queue_capacity + slow-consumer test; RSH-009 LogContext into send_once; RSH-003 one HttpClient shares the provider pause (REQ-SAG-0012 runner side); REQ-QA-018 channel check, 2 s bound; REQ-SAG-0011 to a-10; cause -> cause_detail. SendPermit/LogContext/provider-pause signatures are a-5 section 3's; a-9 names them but does not restate them.
