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

- `sc_agent.send`, `sc_agent.send_batch` and `decidr send` MUST accept an optional per-call send timeout (a Duration), `timeout_secs` (`--timeout-secs` on the CLI).
- `timeout_secs` MUST replace the default 120 s, or a local runner's `request_timeout_secs`, for every send of the call. The exception is a load-on-first-request runner: its sends before the first 2xx MUST use the deadline `load_timeout + timeout_secs`.
- `sc_agent.send_batch` and `decidr send --batch` MUST accept an optional per-call worker count, `workers` (`--workers` on the CLI).
- A worker count below or above the configured `workers` MUST be honored.
- An override MUST apply to its own call only. An absent override MUST mean the configured value.
- A bad override value MUST be returned as an error value:
  - `decidr.invalid_request_timeout` for a bad timeout;
  - `decidr.invalid_workers` for a bad worker count.
- A send that exceeds the timeout MUST be resent under `max_sends` and the start budget.
- `sc_agent.send_batch` MUST return one result per request, each with its input position, while `on_result` receives each line as it finishes.

**MUST NOT statements:**

- An override MUST NOT be clamped.
- An override MUST NOT change the configuration or any other call.
- The start budget MUST NOT cut a send that is in flight.
- `sc_agent.send` MUST NOT accept `workers`.

### Rationale

Different callers in one process need different parallelism and different patience. Without per-call overrides both are fixed by configuration, so consumers refuse or ignore their own flags.

Owner ruling (2026-11-21), Rand Lee: "My words are what I want, not excess control resulting." "Use time structs for time." "always return descriminated unions w/ error info" "I do not require results in request order."

### Success Criteria

**Acceptance Criteria:**

- [ ] A Rust test shows the runner built for a call with `workers` 2 has 2 workers.
- [ ] A Python `send_batch(workers=2)` call finds `workers == 2` in the `runner.stream.start` record written through its injected general Logger.
- [ ] `workers` of 0 or -1 returns an error value `decidr.invalid_workers`.
- [ ] `decidr send --timeout-secs 10` and `--batch --workers 2` reach the effective timeout and worker count, as shown by Rust tests.
- [ ] With `max_sends = 1` and a loopback runner that never answers, `timeout_secs=10` returns a line with:
  - top id `decision.http`;
  - cause `http.network` with `attempts == 1`;
  - a message that begins `timeout failure`.
- [ ] `timeout_secs` of NaN or a negative value returns an error value `decidr.invalid_request_timeout`.
- [ ] Rust tests show the override at both sites:
  - the cloud-provider transport timeout;
  - the local-runner request timeout, where the Laya deadline equals `load_timeout + timeout_secs`.
- [ ] A ManualClock test with `timeout_secs` above 90 s shows the in-flight send is not cut.
- [ ] A call with overrides followed by one without shows the second uses the configured values.
- [ ] `send_batch` returns one result per request, each carrying its input position, while `on_result` sees finish order.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0701
- REQ-SAG-0601
- REQ-SAG-0013
- REQ-SAG-0015
