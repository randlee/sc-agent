# Requirement: A send or batch call may override the send timeout and the worker count for that call only

**Status:** Approved  
**Version:** 0.3.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0702: A send or batch call may override the send timeout and the worker count for that call only

**Status:** Approved  

### Requirement Statement

Definitions:

- The send timeout is the deadline for one HTTP send of a decision request: connect, headers and body.
- The start budget is REQ-SAG-0601's 90 s limit on starting a send, measured from the request's first send.

**MUST statements:**

- `sc_agent.send`, `sc_agent.send_batch` and `decidr send` MUST accept a per-call send timeout in seconds, `timeout_secs` (`--timeout-secs` on the CLI).
- `timeout_secs` MUST be a finite number in 10..=3600. It MUST replace the default 120 s, or a local runner's `request_timeout_secs`, for every send of the call. The exception is a load-on-first-request runner: its sends before the first 2xx MUST use the deadline `load_timeout + timeout_secs`.
- `sc_agent.send_batch` and `decidr send --batch` MUST accept a per-call worker count, `workers` (`--workers` on the CLI). It MUST be an integer in 1..=64.
- A worker count below or above the configured `workers` within that range MUST be honored.
- An override MUST apply to its own call only. An absent override MUST mean the configured value.
- Each override MUST be validated once, in decidr. An invalid Python argument MUST return a tagged result with error information:
  - `error.id` is `decidr.invalid_request_timeout` for a bad timeout;
  - `error.id` is `decidr.invalid_workers` for a bad worker count.

  A range refusal MUST happen before any logger opens or any request is sent, and MUST name the value and the allowed range. `InvalidRequestTimeout` carries the rejected value.
- `decidr send --workers` without `--batch` MUST be refused with `decidr.invalid_workers`.
- A send that exceeds the timeout MUST be resent under `max_sends` and the start budget.
- `sc_agent.send_batch` MUST return its list in input order, while `on_result` receives each line as it finishes.

**MUST NOT statements:**

- An override MUST NOT be clamped.
- An override MUST NOT change the configuration or any other call.
- The start budget MUST NOT cut a send that is in flight.
- `sc_agent.send` MUST NOT accept `workers`.

### Rationale

Different callers in one process need different parallelism and different patience. Without per-call overrides both are fixed by configuration, so consumers refuse or ignore their own flags.

### Success Criteria

**Acceptance Criteria:**

- [ ] A Rust test shows the runner built for a call with `workers` 2 has 2 workers.
- [ ] A Python `send_batch(workers=2)` call finds `workers == 2` in the `runner.stream.start` record written through its injected general Logger.
- [ ] `workers` of 0, 65 or -1 returns a tagged error result with `error.id` `decidr.invalid_workers`, and the loopback runner receives no request.
- [ ] `decidr send --workers 2` without `--batch` exits with `decidr.invalid_workers`.
- [ ] `decidr send --timeout-secs 10` and `--batch --workers 2` reach the effective timeout and worker count, as shown by Rust tests.
- [ ] With `max_sends = 1` and a loopback runner that never answers, `timeout_secs=10` returns a line with:
  - top id `decision.http`;
  - cause `http.network` with `attempts == 1`;
  - a message that begins `timeout failure`.
- [ ] `timeout_secs` of 9.99, 3600.01, NaN or infinity returns a tagged error result with `error.id` `decidr.invalid_request_timeout`.
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
