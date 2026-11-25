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

Definitions: An interrupt is Ctrl-C, SIGINT or SIGTERM delivered to `decidr`, or a `KeyboardInterrupt` raised in Python while `sc-agent-py` is running. The session log is defined in REQ-SAG-0001.

**MUST statements:**

- Ctrl-C, SIGINT or SIGTERM received by `decidr` MUST cancel the running work.
- A `KeyboardInterrupt` in `sc-agent-py` MUST cancel the running work.
- Every request not finished when the interrupt arrives MUST end with `decision.cancelled`.
- The session log MUST be flushed before `decidr` exits or the Python call returns.
- A `KeyboardInterrupt` in `sc-agent-py` MUST be caught inside the Python call.
- After a `KeyboardInterrupt`, the Python call MUST return a result for every request, each unfinished request as a `decision.cancelled` error value.
- `decidr` MUST exit non-zero after an interrupt.

### Rationale

An interrupted batch must leave a complete, readable session log and a clear failure status instead of lost rows and a silent exit.

### Success Criteria

**Acceptance Criteria:**

- [ ] CLI run: sending SIGINT to `decidr` during a batch stops the in-flight requests.
- [ ] CLI run: sending SIGTERM to `decidr` during a batch stops the in-flight requests.
- [ ] CLI run: after SIGINT during a batch, every unfinished request in the output ends with `decision.cancelled`.
- [ ] CLI run: after SIGINT during a batch, the session log holds a row for every request that finished before the signal.
- [ ] Unit test: raising `KeyboardInterrupt` during an `sc-agent-py` batch does not raise out of the call.
- [ ] Unit test: after `KeyboardInterrupt` during an `sc-agent-py` batch, the call returns one result per request, each unfinished one a `decision.cancelled` error value.
- [ ] CLI run: `decidr` exits non-zero after SIGINT during a batch.

### Dependencies

**Requires:**

- REQ-SAG-0104

**Related:**

- REQ-SAG-0701
- REQ-SAG-0005