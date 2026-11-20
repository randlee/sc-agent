# Requirement: Interrupt cancels cleanly

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-20  
**Owner:** Rand Lee  

---

## REQ-SAG-0011: Interrupt cancels cleanly

**Status:** Approved  

### Requirement Statement

Definitions: An interrupt is Ctrl-C, SIGINT or SIGTERM delivered to `decidr`, or a `KeyboardInterrupt` raised in Python while `sc-agent-py` is running.

**MUST statements:**

- Ctrl-C, SIGINT or SIGTERM received by `decidr` MUST cancel the `CancelToken` of the running work.
- A `KeyboardInterrupt` in `sc-agent-py` MUST cancel the `CancelToken` of the running work.
- Every request not finished when the interrupt arrives MUST end with `decision.cancelled`.
- The session log MUST be flushed before `decidr` exits or the Python call returns.
- A `KeyboardInterrupt` in `sc-agent-py` MUST be caught; the Python call MUST return its results, with every unfinished request as a `decision.cancelled` error value.
- `decidr` MUST exit non-zero after an interrupt.
- Exception: a third interrupt received by `decidr` after its logs have opened MUST end `decidr` at once with exit 130, without waiting for the drain or the session-log flush. It is the operator's last resort when a drain cannot finish; requests not yet written may have no session row.

### Rationale

An interrupted batch must leave a complete, readable session log and a clear failure status instead of lost rows and a silent exit.

Owner ruling (2026-11-20), Rand Lee: "Python exceptions need to be caught (return error) so app doesn't crash."

### Success Criteria

**Acceptance Criteria:**

- [ ] Sending SIGINT to `decidr` during a batch makes every unfinished request end with `decision.cancelled`, flushes the session log, and exits non-zero.
- [ ] Sending SIGTERM to `decidr` during a batch has the same result as SIGINT.
- [ ] A third interrupt to `decidr` after its logs have opened exits 130 at once (the first and second only cancel).
- [ ] Raising `KeyboardInterrupt` during an `sc-agent-py` batch cancels the token, ends unfinished requests with `decision.cancelled`, flushes the session log, and returns the results without raising.

### Dependencies

**Requires:**

- REQ-SAG-0104

**Related:**

- REQ-SAG-0701
- REQ-SAG-0005