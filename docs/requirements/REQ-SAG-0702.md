# Requirement: A Run may override the send timeout and the worker count for its own calls only

**Status:** Approved  
**Version:** 0.6.1  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0702: A Run may override the send timeout and the worker count for its own calls only

**Status:** Approved  

### Requirement Statement

Definitions:

- The send timeout is the deadline for one HTTP send of a decision request: connect, headers and body.
- A Run is the wheel's and the Rust crate's run context object (ADR-SAG-0019 decision 14); `decidr send` creates one per command.

**MUST statements:**

- `sc_agent.Run` and `decidr send` MUST accept an optional send timeout (a Duration), `timeout_secs` (`--timeout-secs` on the CLI).
- `timeout_secs` MUST replace the default 120 s for every send of the Run.
- `sc_agent.Run` and `decidr send --batch` MUST accept an optional worker count, `workers` (`--workers` on the CLI).
- A worker count below or above the configured `workers` MUST be honored.
- An override MUST apply to its own Run only. An absent override MUST mean the configured value.
- A bad override value MUST be returned as an error value by every call through that Run:
  - `decidr.invalid_request_timeout` for a bad timeout;
  - `decidr.invalid_workers` for a bad worker count.
- A send that exceeds the timeout MUST be resent up to the fixed send count of 5 sends in all, the first included (REQ-SAG-0601).
- `run.send_batch` MUST return one result per request, each with its input position, while `on_result` receives each line as it finishes.

**MUST NOT statements:**

- An override MUST NOT be clamped.
- An override MUST NOT change the configuration or any other Run.
- `run.send` and `run.send_batch` MUST NOT accept `timeout_secs` or `workers`.

### Rationale

Different callers in one process need different parallelism and different patience. Without per-call overrides both are fixed by configuration, so consumers refuse or ignore their own flags.

Owner ruling (2026-11-21), Rand Lee: "Use time structs for time." "always return descriminated unions w/ error info" "I do not require results in request order."

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

Owner ruling 2026-11-24 (plan review, ADR-SAG-0003): values that do not vary between the calls of one command, the send timeout and the worker count among them, are set once on the Run; per call only the request, model, agent role, cache override and batch callback are passed (ADR-SAG-0019 decision 14).

Owner ruling 2026-11-24 (plan review; supersedes `model` in the per-call list above): one model per Run, chosen by the caller; the model is set once on the Run and no call passes a model.

### Success Criteria

**Acceptance Criteria:**

- [ ] A Rust test shows a Run with `workers` 2 runs 2 workers.
- [ ] `workers` of 0 or -1 returns an error value `decidr.invalid_workers`.
- [ ] `decidr send --timeout-secs 10` and `--batch --workers 2` reach the effective timeout and worker count, as shown by Rust tests.
- [ ] With a loopback runner that never answers, `timeout_secs` returns a line with the error id `http.network` and `attempts` equal to the sends made.
- [ ] `timeout_secs` of NaN or a negative value returns an error value `decidr.invalid_request_timeout`.
- [ ] Rust tests show the override is the transport timeout.
- [ ] A Run with overrides followed by a Run without shows the second uses the configured values.
- [ ] `run.send_batch` returns one result per request, each carrying its input position, while `on_result` sees finish order.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0701
- REQ-SAG-0601
- REQ-SAG-0013
- REQ-SAG-0015
