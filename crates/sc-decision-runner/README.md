# sc-decision-runner

The decision runner of sc-agent. `DecisionRunner` is the one implementation of `sc_decision_schema::Decision`. It runs requests over any `Arc<dyn Backend>` and one `sc_agent_http::HttpClient`, with worker threads, holdoffs in the queue, cancellation and one session row per result.

It sits in the runner layer, above `sc-decision-schema`. It launches decision models only, and it depends on no decision-backend crate.

## What it provides

- `DecisionRunner`:
  - `DecisionRunner::new(backend, http, session, general, config)`, and `with_clock` for tests.
  - `decide` runs one request with one worker. `decide_stream` runs many: one feeder thread and `RunnerConfig::workers` worker threads. Results come back fastest first, failures included. A send never runs on the caller's thread.
  - `flush_session_log` flushes the session log and reports rows that were refused or lost.
- `RunnerConfig` (`RunnerConfig::new(workers)`, `from_config(&AgentConfig)`, `workers`, `queue_capacity`), `JOBS_PER_WORKER` and `MAX_WORKERS`.
- `RunnerError`.

Behaviour:

- A busy or timed-out send waits in the queue until its holdoff ends, not in a worker.
- A reply that does not match the schema is resent unchanged, counted against the same `max_sends` and 90 s budget.
- Cancelling the caller's `CancelToken` ends every unfinished request at once with `decision.cancelled`, in-flight sends included. A late reply is discarded.
- Exactly one result and one session row per request.

## Usage

```rust,ignore
use std::sync::Arc;

use sc_agent_types::CancelToken;
use sc_decision_runner::{DecisionRunner, RunnerConfig};
use sc_decision_schema::Decision;

// backend: Arc<dyn Backend>; http: HttpClient; session and general: the loggers.
let runner = DecisionRunner::new(backend, http, session, general, RunnerConfig::from_config(&config)?);

let cancel = CancelToken::new();
let result = runner.decide(request, &cancel);

let results = runner.decide_stream(Box::new(requests.into_iter()), &cancel);
for result in results {
    println!("request {}: ok = {}", result.position, result.outcome.is_ok());
}
runner.flush_session_log()?;
```

## Depends on

- `sc-agent-config`, `sc-agent-http`, `sc-agent-log`, `sc-agent-types` and `sc-decision-schema`.

Layer rule: a crate depends only on lower layers (ADR-SAG-0004). It uses sync threads, `Mutex` and `Condvar` only: no tokio, no sleep.

## Requirements and decisions

- REQ-SAG-0701: batches of decision requests run in parallel.
- REQ-SAG-0601: busy resend and holdoff.
- REQ-SAG-0012: provider-wide busy pause.
- REQ-SAG-0901: a schema mismatch is resent.
- REQ-SAG-0104: cancel token.
- REQ-SAG-0009 and REQ-SAG-0010: no tokio, no sleep.
- NFR-SAG-0001: session logging at 40,000 sessions per task.
- ADR-SAG-0001, ADR-SAG-0002 and ADR-SAG-0004: one backend trait, JSONL session logs, crate layering.
