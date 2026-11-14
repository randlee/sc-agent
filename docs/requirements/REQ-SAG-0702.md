# Requirement: A send or batch call may override the send timeout and the worker count for that call only

**Status:** Proposed  
**Version:** 0.2.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-14  
**Owner:** Rand Lee  

---

## REQ-SAG-0702: A send or batch call may override the send timeout and the worker count for that call only

**Status:** Proposed  

### Requirement Statement

Definitions:

- The send timeout is the deadline for one HTTP send of a decision request: connect, headers and body.
- The start budget is REQ-SAG-0601's limit on starting sends, measured from the request's first send.
- The request-timeout bounds are REQ-SAG-0015's bounds on a request timeout. The `workers` bounds are REQ-SAG-0013's bounds on `workers`. The remote default request timeout is REQ-SAG-0015's default request timeout for remote providers.

**MUST statements:**

- `sc_agent.send`, `sc_agent.send_batch` and `decidr send` MUST accept a per-call send timeout in seconds, `timeout_secs` (`--timeout-secs` on the CLI).
- `timeout_secs` MUST be a finite number within the request-timeout bounds. It MUST replace the remote default request timeout, or a local runner's `request_timeout_secs`, for every send of the call. The exception is a load-on-first-request runner: its sends before the first 2xx MUST use the deadline `load_timeout + timeout_secs`.
- `sc_agent.send_batch` and `decidr send --batch` MUST accept a per-call worker count, `workers` (`--workers` on the CLI). It MUST be an integer within the `workers` bounds.
- A worker count below or above the configured `workers` within that range MUST be honored.
- An override MUST apply to its own call only. An absent override MUST mean the configured value.
- Each override MUST be validated once, in decidr. Python arguments MUST be refused with `SagError`:
  - `decidr.invalid_request_timeout` for a bad timeout;
  - `decidr.invalid_workers` for a bad worker count.

  A refusal MUST happen before any logger opens or any request is sent, and MUST name the value and the allowed range.
- A `bool` MUST be refused for both overrides. (An unknown keyword, such as `workers` on `send`, raises Python's own `TypeError`.)
- `decidr send --workers` without `--batch` MUST be refused with `decidr.invalid_workers`.
- `timeout_secs` with an injected transport MUST be refused with `decidr.invalid_request_timeout`.
- A send that exceeds the timeout MUST be resent under `max_sends` and the start budget.
- `sc_agent.send_batch` MUST return its list in input order, while `on_result` receives each line as it finishes.

**MUST NOT statements:**

- An override MUST NOT be clamped.
- An override MUST NOT change the configuration or any other call.
- The start budget MUST NOT cut a send that is in flight.
- `sc_agent.send` MUST NOT accept `workers`.

### Rationale

Different callers in one process need different parallelism and different patience. Today both are fixed by configuration, so consumers refuse or ignore their own flags. `timeout_secs` was added on the phase b stack (agt-b-1.8) and is recorded here. `workers`, the CLI flags, the injected-transport rule and the batch order are new.

### Success Criteria

**Acceptance Criteria:**

- [ ] A Rust test shows the runner built for a call with `workers` 2 has 2 workers.
- [ ] A Python `send_batch(workers=2)` call reads `workers == 2` from the `runner.stream.start` event passed to its `general_log` callable.
- [ ] `workers` of 0, 65, -1, 2.5, `"2"` or `True` raises `SagError` `decidr.invalid_workers`, and the loopback runner receives no request.
- [ ] `decidr send --workers 2` without `--batch` exits with `decidr.invalid_workers`.
- [ ] `decidr send --timeout-secs 10` and `--batch --workers 2` reach the effective timeout and worker count, as shown by Rust tests.
- [ ] With `max_sends = 1` and a loopback runner that never answers, `timeout_secs=10` returns a line with:
  - top id `decision.http`;
  - cause `http.network` with `attempts == 1`;
  - a message that begins `timeout failure`.
- [ ] `timeout_secs` of 9.99, 3600.01, NaN, infinity or `True` raises `SagError` `decidr.invalid_request_timeout`.
- [ ] `timeout_secs` with an injected transport raises `decidr.invalid_request_timeout`.
- [ ] Rust tests show the override at both sites:
  - the cloud-provider transport timeout;
  - the local-runner request timeout, where the Laya deadline equals `load_timeout + timeout_secs`.
- [ ] A ManualClock test with `timeout_secs` above 90 s shows the in-flight send is not cut.
- [ ] A call with overrides followed by one without shows the second uses the configured values.
- [ ] `send_batch` returns its list in input order while `on_result` sees finish order.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0701
- REQ-SAG-0601
- REQ-SAG-0013
- REQ-SAG-0015
- REQ-SAG-0015
