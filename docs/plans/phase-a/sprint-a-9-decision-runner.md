<!-- Generated from bead agt-4p2.9 for file-based reviewers. The bead is the source of truth: edit the bead, then re-export. -->

# Sprint A-9: sc-decision-runner
- id: `a-9` (bead `agt-4p2.9`)
- branch: `sprint/a-9-decision-runner`
- depends on: agt-4p2.2
- copy script: `docs/plans/phase-a/copy/a-9.sh`

## Description

Implements Decision for any Arc<dyn Backend> over an HttpClient: queue, workers, holdoff in the queue, cancellation via CancelToken hooks, schema-mismatch resend, one session row per result, results fastest first, failed requests returned while the stream continues. Records: REQ-SAG-0701, REQ-SAG-0601, REQ-SAG-0202, REQ-SAG-0901, ADR-SAG-0001.

## Design

# Sprint A-9: sc-decision-runner (bead agt-4p2.9)

Governing: REQ-SAG-0701 (batches in parallel, workers default 8, fastest first, failures returned, cancel), REQ-SAG-0601 (holdoff waits in the queue, cancel ends it at once, no sleep), REQ-SAG-0602 (quota passed through), REQ-SAG-0901 (a reply that does not match the required schema is resent unchanged up to max_sends, then returned as decision.schema_mismatch), REQ-SAG-0202 (digest recorded with the response), ADR-SAG-0001 (one Decision trait over any Backend), ADR-SAG-0002 / NFR-SAG-0001 (one session row per session), ADR-SAG-0004 (top decision layer: depends on sc-decision-schema and the layers below; no backend crate; decisions only), REQ-SAG-0008. Also binding: REQ-SAG-0001, 0002, 0004, 0007, 0009, 0010.
Owner decisions: owner-decision-trait, owner-busy-resend, owner-architecture, owner-no-sleep, owner-lowlevel-no-tokio, owner-local-decision-crate, owner-error-contract, owner-workers-default.

Copy script: `docs/plans/phase-a/copy/a-9.sh` (dry run 2026-11-09, reconciliation: 8 copy steps, 4 files, 546 lines). Target: `crates/sc-decision-runner/`.

## Shape in one paragraph

`DecisionRunner` owns one `Arc<dyn Backend>` (A-2), one `HttpClient` (A-5: transport, `ResendPolicy`, general logger), a `SessionLogger` and a `GeneralLogger` (A-8), and implements `Decision`. `decide_stream` starts one feeder thread (pulls the caller's iterator into a bounded `JobQueue`) and `workers` worker threads (default 8, `AgentConfig.workers`). A worker takes a ready job; on its first attempt it computes the question digests and encodes the request once; it calls `HttpClient::send_once`. `Next::ResendAt(t)` puts the job back in the queue's held heap and frees the worker. `Next::Done(response)` goes to `backend.decode(response, &request)`; `Err(DecisionError::SchemaMismatch)` puts the job back as ready with the same `Attempts` and the same encoded bytes; every send of a request (busy, timeout and schema resends alike) counts against one `max_sends` and one 90 s budget, and when either runs out after a mismatch the job finishes with that `SchemaMismatch`, whose detail names what did not match (REQ-SAG-0901); anything else finishes the job. Idle workers wait with `Condvar::wait_timeout` to the earliest holdoff end; a `CancelToken::on_cancel` hook (A-1) cancels the queue and wakes every waiter at once. Each finished job writes exactly one session row, then its `DecisionResult` goes to a bounded result queue that the returned iterator drains, so results come back fastest first. `decide` runs one request on the caller's thread and holds off with `CancelToken::wait_until`. A session-log write failure never replaces the decision result: the row's Err is logged to the general log and counted by the `SessionLogger`, and `flush_session_log` reports it.

## 1. Copy list (source `sc-agent-ref@dce239a` -> `crates/sc-decision-runner/`)

| # | Source path:lines | Target | What | Required edits after copying |
|---|---|---|---|---|
| C1 | `crates/sc-jev-host/LICENSE` | `LICENSE` | MIT | none |
| C2 | `crates/sc-jev-host/src/stream/pool.rs:32-200` | `src/queue.rs` | `Skipped`, `BoundedQueue<T>` (Mutex + Condvars, `close`, `discard`, poison recovered), `PushError`, `Popped` | `BoundedQueue` stays as the result queue: drop `pushed`/`popped`, `PushError::Full` and the timed push/pop (always block; woken by `close`/`discard`). `Skipped` keeps `Cancelled` and `Panicked`. Add `JobQueue<J>` (N2). |
| C3 | `crates/sc-jev-host/src/stream/pool.rs:202-297` | `src/pool.rs` | `Counters`, `WorkerPool::start/submit/close_input/cancel/recv` | Rename `Pool`. `RunFn` becomes `Fn(J) -> Step<J, A>`; delete `LostFn`, `Counters.running`, `workers`, `queue_capacity`. `start` uses `thread::Builder::new().name("sc-decision-worker").spawn` and returns `Result<Self, std::io::Error>`. `cancel()` cancels the `JobQueue` only, never the caller's token. `recv()` = `results.pop()`. |
| C4 | `crates/sc-jev-host/src/stream/pool.rs:337-417` | `src/pool.rs` | `join`, `abandon`, `Drop`, `Worker`, `Exit` guard, `work` loop with `catch_unwind` | `Drop`/`abandon`: `jobs.cancel()` + `results.discard()`. `work`: `Step::HoldOff { until, job }` -> `jobs.hold(job, until)`; `Done(a)` -> push; `jobs.done()` after every run. `lost` is gone. `#[cfg(test)] mod tests;`. |
| T1 | `crates/sc-jev-host/src/stream/pool/tests.rs:1-65, 76-91, 99-125, 213-283` | `src/pool/tests.rs` | `Rendezvous`, `pool_of`, `next`, `drain`; tests: workers overlap (2 and 8), cancel skips queued and refuses new, a panicking job is answered | Delete the sleep-polling `wait_until`/`join` helpers; use a "started" `mpsc` signal and `pool.join()`. `next` uses blocking `recv` (failsafe via `mpsc::recv_timeout` on a helper thread). Port closures to `Step::Done`. |

Not copied: `stream/mod.rs` (JSONL wire format, exit codes, `serve_jsonl`, `Batch`: CLI concerns, A-10; its `abandon` sleeps); `Concurrency` (`MAX = 64` is in no record); `stream/log.rs` (replaced by `GeneralLogger::write`); introspection counters; pool tests at `tests.rs:66-75` (sleep) and `126-212, 284-365`.

## 2. Interfaces consumed (exact; defined by A-1, A-2, A-4, A-5, A-8)

```rust
// A-1: CancelToken { is_cancelled, wait_until(Instant) -> Wait::{Cancelled, DeadlineReached}, on_cancel(Arc<dyn Fn() + Send + Sync>) -> CancelHook }; SessionId::new(); ErrorId, ErrorDetail
// A-2: DecisionRequest, SystemOneResponse, OrderedMap, QuestionDigest, question_digests(&DecisionRequest),
//      DecisionResult { session_id, position, digests, outcome: Result<SystemOneResponse, DecisionError> },
//      trait Decision, trait Backend { encode(&DecisionRequest) -> Result<HttpRequest, DecisionError>;
//                                     decode(HttpResponse, &DecisionRequest) -> Result<SystemOneResponse, DecisionError> },
//      DecisionError::{Http, SchemaMismatch, Cancelled { sends }, Shutdown { reason }, ..}
// A-4: AgentConfig { workers, max_sends, .. }
// A-5: HttpClient::send_once(&HttpRequest, &mut Attempts, &dyn Fn() -> Instant) -> Result<Next, HttpError>; Attempts; Next::{Done, ResendAt}
// A-8: SessionLogger::write(SessionId, Map) / flush(); GeneralLogger::write(Level, target, action, Map); LogError
```

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
pub struct DecisionRunner {
    backend: Arc<dyn Backend>,
    http: HttpClient,
    session: SessionLogger,
    general: GeneralLogger,
    config: RunnerConfig,
}
impl DecisionRunner {
    pub fn new(backend: Arc<dyn Backend>, http: HttpClient, session: SessionLogger, general: GeneralLogger, config: RunnerConfig) -> Self;
    /// Flushes the session log; RunnerError::SessionLog(log.rows_lost | log.flush) when any row was refused or lost.
    pub fn flush_session_log(&self) -> Result<(), RunnerError>;
}
impl Decision for DecisionRunner {
    /// One request on the caller's thread; holdoff = cancel.wait_until(until).
    fn decide(&self, request: DecisionRequest, cancel: &CancelToken) -> DecisionResult;
    /// Feeder + config.workers workers; results in finish order; exactly one result per request.
    fn decide_stream<'a>(&'a self, requests: Box<dyn Iterator<Item = DecisionRequest> + Send + 'a>, cancel: &CancelToken)
        -> Box<dyn Iterator<Item = DecisionResult> + Send + 'a>;
}

// crate-private
pub(crate) enum Step<J, A> { Done(A), HoldOff { until: Instant, job: J } }   // a schema resend is HoldOff { until: now }
pub(crate) struct JobQueue<J> { state: Mutex<JobState<J>>, changed: Condvar, capacity: usize }
struct JobState<J> { ready: VecDeque<J>, held: BinaryHeap<Held<J>>, running: usize, closed: bool, cancelled: bool, seq: u64 }
struct Held<J> { until: Instant, seq: u64, job: J }       // Ord: earliest `until`, then `seq`
pub(crate) enum Taken<J> { Run(J), Cancelled(J), Finished }
impl<J> JobQueue<J> {
    fn submit(&self, job: J) -> Result<(), J>;  // blocks while ready+held+running >= capacity; Err(job) once closed or cancelled
    fn hold(&self, job: J, until: Instant);     // never blocks
    fn take(&self) -> Taken<J>;                 // ready first; due held jobs move to ready; else wait_timeout(earliest - now) or wait()
    fn done(&self);                             // running -= 1; notify_all
    fn close(&self);                            // input ended
    fn cancel(&self);                           // take() now yields Cancelled(job) for every ready and held job; notify_all
}
struct Job {
    session_id: SessionId, position: u64, request: DecisionRequest,
    digests: OrderedMap<String, QuestionDigest>,  // computed on the first attempt
    http: Option<HttpRequest>,                    // encoded once; every send (busy or schema resend) sends the same bytes
    attempts: Attempts,                           // one per request: busy and schema resends share it
    sends: u32, schema_resends: u32,
}
```

Session row (`src/row.rs`, one per result: ok, failed and cancelled; `SessionLogger` adds `session_id`, `repo`, `project`, `agent_role`):

```text
{ "position": <u64>, "model": <request model string>, "sends": <u32>, "schema_resends": <u32>,
  "outcome": "ok" | "error",
  "digests": { "<question id>": "<sc-sha hex>", ... },
  "response": <SystemOneResponse, unrounded>,           // ok
  "error": <ErrorDetail of the DecisionError> }          // error: id, message and every lower id and message
```

General log (`GeneralLogger::write`, target `sc_decision_runner`): `runner.stream.start` (workers), `runner.stream.end` (results, failed), `runner.worker.stopped` (reason), `runner.session_log_failed` (session_id, error detail). No request or response content.

## 4. Errors

Results carry `DecisionError` (A-2), with the runner-level failures in it: `Cancelled { sends }` for every request not finished when the token is cancelled (queued, held off or refused at submit), `Shutdown { reason }` when no worker could start or a worker stopped (a caught panic's payload text kept). `Http`, `SchemaMismatch` (after `max_sends` mismatching replies), `Backend` and the decode errors pass through unchanged.

The crate's own union covers its one fallible method:

```rust
#[derive(Debug)]
#[non_exhaustive]
pub enum RunnerError {
    /// The session log could not be flushed or lost rows (LogError kept unchanged: log.flush, log.rows_lost).
    SessionLog(LogError),                       // "runner.session_log"
}
impl ErrorId for RunnerError {}                // cause = LogError detail; source() = Some(inner)
```

Lints: workspace `panic`/`unwrap_used`/`expect_used`/`indexing_slicing` deny (A-1); `catch_unwind` around backend and transport calls is the only panic handling.

## 5. New code (not in the copy)

- N1 `Cargo.toml`, `README.md`, `src/lib.rs`, `src/config.rs`, `src/error.rs`.
- N2 `JobQueue<J>` in `src/queue.rs` (~90 lines).
- N3 `src/runner.rs` (~170 lines): `DecisionRunner`, the step fn (prepare on first attempt -> `send_once` -> `ResendAt` hold / `Done` -> `decode` -> `SchemaMismatch` hold now / finish), the skipped fn (`Cancelled { sends }`), the feeder, the `decide` loop with `cancel.wait_until`, the `on_cancel` hook calling `jobs.cancel()` (the `CancelHook` lives as long as the stream), the result iterator (Drop = `abandon`, never cancels the caller's token).
- N4 `src/row.rs` (~30 lines).
- Config keys used (A-4): `workers`, `max_sends` (through the caller's `ResendPolicy` in `HttpClient`). No new keys.

## 6. Tests (in-process test backends and transports; delays wait on a Condvar or `CancelToken::wait_until`, never sleep)

| Test | Proves |
|---|---|
| `forty_requests_parallel`: 40 requests, transport answers each after 200 ms; default config finishes in < 40 x 200 ms / 4 and a `Rendezvous(8)` in the transport is reached | REQ-SAG-0701 AC1 |
| `one_worker_is_serial` | REQ-SAG-0701 AC2 |
| `failed_request_returned_rest_continue`: request 7 of 20 fails encode; 20 results, #7 is Err with its own session_id | REQ-SAG-0701 AC3 |
| `fastest_first` | REQ-SAG-0701 (finish order) |
| `cancel_running_batch`: every unfinished request returns decision.cancelled; results = requests submitted | REQ-SAG-0701 AC4 |
| `holdoff_frees_the_worker`: workers = 1; A gets `ResendAt(now + 3 s)`, B is answered during A's holdoff | REQ-SAG-0601 (wait in the queue) |
| `cancel_ends_holdoff_at_once`: a request held off 30 s; cancel -> decision.cancelled well under 30 s | REQ-SAG-0601 AC4 |
| `decide_cancel_during_holdoff` | REQ-SAG-0601 AC4 (single path) |
| `busy_verdict_passed_through`: busy every time -> decision.http with cause http.provider_busy (sends, 5); transport called 5 times | REQ-SAG-0601 AC1, REQ-SAG-0602 |
| `same_bytes_each_send`: byte-identical HttpRequests on every resend; encode called once | REQ-SAG-0601 |
| `schema_mismatch_resent_once`: a test backend whose decode returns SchemaMismatch (missing probabilities) on the first reply and a valid answer on the second -> Ok with the valid answer, 2 sends, schema_resends 1 in the row | REQ-SAG-0901 AC2 |
| `schema_mismatch_after_max_sends`: a backend that always mismatches -> decision.schema_mismatch after 5 decoded replies (default), naming what did not match; `max_sends = 2` -> 2 | REQ-SAG-0901 AC3 |
| `cancel_ends_schema_resends`: a backend that always mismatches; cancel -> decision.cancelled | cancel stops everything |
| `one_row_per_result`: 50 requests incl. failed and cancelled -> 50 rows with matching session_ids; ok rows carry digests and response; error rows carry the ErrorDetail chain | NFR-SAG-0001, REQ-SAG-0202 |
| `three_questions_three_digests` | REQ-SAG-0202 AC3 |
| `session_log_failure_keeps_result`: injected session logger shut down -> the result is still Ok and unchanged; `runner.session_log_failed` in the general log; `flush_session_log` -> runner.session_log with cause log.rows_lost | result never replaced; nothing lost silently |
| `panic_in_backend_is_a_result`: decision.shutdown with the payload text | REQ-SAG-0002 |
| `dropping_the_stream_does_not_cancel_the_token` | token ownership |
| `no_tokio`: `cargo tree -p sc-decision-runner` has no tokio | REQ-SAG-0009 |
| ported pool tests (T1) | pool mechanics |

## 7. Dependencies

- `sc-agent-types` (CancelToken, SessionId, ErrorId), `sc-agent-config` (AgentConfig, DEFAULT_WORKERS), `sc-decision-schema` (Decision, Backend, DecisionRequest/Result/Error, digests), `sc-agent-http` (HttpClient, HttpRequest, Attempts, Next), `sc-agent-log` (SessionLogger, GeneralLogger, Level, LogError) — all path.
- `serde_json = "1"` (row payload).
- No backend crate (backends arrive as `Arc<dyn Backend>`), no sc-observability, no tokio, no thiserror.

## Acceptance criteria

- [ ] Copy first: the sprint's first commit is the unchanged output of `docs/plans/phase-a/copy/a-9.sh` (sc-agent-ref@dce239a), committed before any edit; every change needed to make that code build and pass comes in later commits (owner-copy-mechanically).
- [ ] REQ-SAG-0701 AC1: a batch of 40 requests against a test transport that answers each after a delay finishes in about one eighth of the serial time with the default 8 workers (8 transport calls observed in flight at once).
- [ ] REQ-SAG-0701 AC2: with workers = 1 the same batch runs one request at a time; workers come from AgentConfig.workers, default 8.
- [ ] REQ-SAG-0701 AC3: a batch with one failing request returns that error tagged with its session ULID and returns every other result; results come back in finish order, each with its session ULID and input position.
- [ ] REQ-SAG-0701 AC4: cancelling a running batch returns decision.cancelled for every unfinished request; one result per request, always.
- [ ] REQ-SAG-0601: sc-decision-runner resends a request that gets HTTP 429, 502 or 529 until the configurable max_sends (default 5, set in the HttpClient ResendPolicy) is reached, and sends the same encoded bytes each time. Test: a transport answering 529 four times then 200 yields the answer; one answering 529 every time stops after max_sends sends. A held-off request waits in the JobQueue, not in a worker (with 1 worker a second request is answered during the first one's holdoff); the wait is a Condvar timed wait woken by a CancelToken::on_cancel hook; no sleep.
- [ ] REQ-SAG-0601 AC4: cancelling a held-off request ends its wait at once and returns decision.cancelled (decide_stream and decide).
- [ ] REQ-SAG-0601/0602: the runner resends only on Next::ResendAt, sends the same encoded bytes each time, and returns http.provider_busy / http.quota_exhausted / http.network unchanged inside decision.http.
- [ ] REQ-SAG-0901 AC2: a backend that first returns decision.schema_mismatch (a reply missing probabilities) and then a valid answer yields the valid answer after one resend; resends stop at once on cancel.
- [ ] REQ-SAG-0901 AC3: a backend that never matches yields decision.schema_mismatch, with what did not match, after max_sends sends in total (default 5); a backend answering 529 twice and then never matching stops after 5 sends, not more.
- [ ] NFR-SAG-0001 / ADR-SAG-0002: exactly one session row per result (ok, failed and cancelled), each with its session ULID and the identity fields session_id, repo, project, agent_role; error rows carry the DecisionError ErrorDetail chain.
- [ ] REQ-SAG-0202: each ok row records one sc-sha question digest per question with the response (three questions -> three digests).
- [ ] ADR-SAG-0001: every backend sc-decision-runner drives implements sc_decision_schema::Backend (the sans-IO trait: encode a Jev request into an HTTP request, decode an HTTP response into a Jev response); DecisionRunner holds only an Arc<dyn sc_decision_schema::Backend> and implements the one Decision trait (no associated error) over it and an HttpClient. Test: the runner tests drive two different Backend implementations through the same DecisionRunner.
- [ ] ADR-SAG-0004: sc-decision-runner sits in the runner layer; its sc-agent crate dependencies are sc-agent-types, sc-agent-config, sc-agent-secrets, sc-agent-log, sc-agent-http and sc-decision-schema, and it depends on no decision-backend crate, checked by `cargo tree -p sc-decision-runner`.
- [ ] REQ-SAG-0002: every error returned by a public runner function (decide, the decide_stream results, flush_session_log) carries the error id and message of each lower-layer error that caused it: HttpError inside decision.http, LogError inside runner.session_log, and the backend's own error. Test: inject a failure in the transport, in the session log and in backend decode, and assert that failure's id and message are in the returned DecisionError chain. Runner-level failures are decision.cancelled and decision.shutdown (a backend panic becomes decision.shutdown); a session-log write failure never replaces the decision result; no thread::spawn (Builder::spawn only); clippy with the workspace denies passes.
- [ ] Dropping the result iterator never cancels the caller's CancelToken.
- [ ] REQ-SAG-0009/0010: cargo tree shows no tokio; a search of non-test source finds no sleep call.
- [ ] REQ-SAG-0010: the runner waits out holdoffs and idle workers with CancelToken::wait_until and on_cancel, never a sleep function; enforced by the workspace clippy disallowed-methods rule.
- [ ] REQ-SAG-0201: sc-decision-runner defines no Rust type that maps to a field of the TypeSafe Jev API (POST /v1/systemone, GET /v1/models); every such type it uses (Request, Response, Backend, Decision, DecisionError) is imported from sc-decision-schema. Check: a search of crates/sc-decision-runner/src finds no serde type for an API field.
- [ ] ANC-007: sc-decision-runner writes runner.stream.start, runner.stream.end, runner.worker.stopped and runner.session_log_failed to the GeneralLogger passed to DecisionRunner::new; a test injects an in-memory GeneralLogger, runs a stream with one failing request and finds those records.

## Notes

Closed 2026-11-09 by owner answers (search-for-truth step 7):
- Q1 queue capacity: 4 x workers, a constant (assistant).
- Schema resend limit: max_sends decoded replies, then decision.schema_mismatch (REQ-SAG-0901).
- Result correlation: session ULID and input position (REQ-SAG-0701).
No open questions.
