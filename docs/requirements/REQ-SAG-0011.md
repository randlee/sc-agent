# Requirement: Interrupt cancels cleanly

**Status:** Approved  
**Version:** 0.5.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0011: Interrupt cancels cleanly

**Status:** Approved  

### Requirement Statement

Definitions: `decidr` is the decision CLI, and `sc-agent-py` is the Python wheel that holds the decidr API. An interrupt is Ctrl-C, SIGINT or SIGTERM delivered to `decidr`, or a `KeyboardInterrupt` raised in Python while `sc-agent-py` is running. The session log is the JSONL file `sessions.jsonl` that holds one row for each request sent to a model (REQ-SAG-0005). A batch is the set of requests sent by one `decidr` command or one `sc-agent-py` call. The Python call is the `sc-agent-py` call that sent the batch. An unfinished request is a request of the batch that has no result when the interrupt arrives. `decision.cancelled` is the error id given to a cancelled request.

**MUST statements:**

- When `decidr` receives an interrupt, it MUST cancel the batch.
- When a `KeyboardInterrupt` is raised in `sc-agent-py`, it MUST cancel the batch.
- After an interrupt, every unfinished request MUST end with `decision.cancelled`.
- The session log MUST hold every row of the batch when `decidr` exits or the Python call returns.
- A `KeyboardInterrupt` in `sc-agent-py` MUST be caught inside the Python call.
- After a `KeyboardInterrupt`, the Python call MUST return one result per request of the batch.
- `decidr` MUST exit non-zero after an interrupt.

### Rationale

An interrupted batch must leave a complete, readable session log and a clear failure status instead of lost rows and a silent exit.

### Success Criteria

**Acceptance Criteria:**

- [ ] CLI run: sending SIGINT to `decidr` during a batch stops the in-flight requests.
- [ ] CLI run: sending SIGTERM to `decidr` during a batch stops the in-flight requests.
- [ ] CLI run: after SIGINT, and in a separate run after SIGTERM, during a batch, every unfinished request in the output ends with `decision.cancelled`.
- [ ] CLI run: after SIGINT, and in a separate run after SIGTERM, during a batch, the session log holds a row for every request that finished before the signal.
- [ ] Unit test: raising `KeyboardInterrupt` during an `sc-agent-py` batch stops the in-flight requests.
- [ ] Unit test: raising `KeyboardInterrupt` during an `sc-agent-py` batch does not raise out of the call.
- [ ] Unit test: after `KeyboardInterrupt` during an `sc-agent-py` batch, the session log holds a row for every request that finished before the interrupt.
- [ ] Unit test: after `KeyboardInterrupt` during an `sc-agent-py` batch, the call returns one result per request, each unfinished one a `decision.cancelled` error value.
- [ ] CLI run: `decidr` exits non-zero after SIGINT, and in a separate run after SIGTERM, during a batch.

### Dependencies

**Requires:**

- REQ-SAG-0104

**Related:**

- REQ-SAG-0701
- REQ-SAG-0005