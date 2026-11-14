# Requirement: A batch or send call may override workers and the send timeout for that call only

**Status:** Proposed  
**Version:** 0.1.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-14  
**Owner:** Rand Lee  

---

## REQ-SAG-0702: A batch or send call may override workers and the send timeout for that call only

**Status:** Proposed  

### Requirement Statement

Definitions: The send timeout is the deadline for one HTTP send of a decision request: connect, headers and body. The start budget is REQ-SAG-0601's 90 s limit on starting a send, measured from the request's first send.

**MUST statements:**

- `sc_agent.send_batch` and `decidr send --batch` MUST accept a per-call worker count: `workers` and `--workers`.
- `sc_agent.send`, `sc_agent.send_batch` and `decidr send` MUST accept a per-call send timeout in seconds: `timeout` and `--timeout`.
- An override MUST apply to that call only. It MUST NOT change the configuration or any other call.
- An absent override MUST mean the configured value.
- `workers` MUST be an integer in 1..=64. A value below or above the configured `workers` within that range MUST be honored.
- `timeout` MUST be a finite number of seconds in (0, 3600]. It MUST replace the default 120 s or the local runner's `request_timeout_secs` for every send of the call, resends included.
- For a load-on-first-request runner, the per-request deadline MUST become `load_timeout + timeout`.
- A send that exceeds `timeout` MUST be resent under `max_sends` and the start budget, as for any other timeout.
- An out-of-range or ill-typed override MUST be refused with the error `decidr.invalid_option`, before anything is sent. The error MUST name the option, the value and the allowed range.

**MUST NOT statements:**

- An override value MUST NOT be clamped.
- The start budget MUST NOT cut a send that is in flight.

### Rationale

Different callers in one process need different parallelism and different patience, for example a quick interactive check next to a long batch. Today both are fixed by configuration, so consumers refuse or ignore their own flags.

### Success Criteria

**Acceptance Criteria:**

- [ ] In a batch of 6 with `workers=2` against a loopback runner that holds every request until the test releases it, the runner records at most, and at some point exactly, 2 requests in flight.
- [ ] `workers` of 0, 65 or `True` raises `SagError` `decidr.invalid_option`, and the runner receives no request.
- [ ] With `max_sends = 1` and a loopback runner that never answers, `timeout=0.2` returns a result whose error chain ends in `http.network` (timeout) with `attempts == 1`.
- [ ] `timeout` of 0, -1, NaN, infinity or 3601 raises `SagError` `decidr.invalid_option`.
- [ ] A Rust test shows the effective request timeout equals the override for a cloud provider and for a local runner, and the Laya deadline equals `load_timeout + timeout`.

### Dependencies

**Related:**

- ADR-SAG-0019
- ADR-SAG-0004
- REQ-SAG-0701
- REQ-SAG-0601
- REQ-SAG-0013
- REQ-SAG-0015
