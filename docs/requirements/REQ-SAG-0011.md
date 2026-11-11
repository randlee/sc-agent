# Requirement: Interrupt cancels cleanly

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-10  
**Owner:** Rand Lee  
**Revision:** 0.2.0 (2026-11-10, owner): a third interrupt after the logs open ends `decide` at once (operator last resort).  

---

## REQ-SAG-0011: Interrupt cancels cleanly

**Status:** Approved  

### Requirement Statement

Definitions: An interrupt is Ctrl-C, SIGINT or SIGTERM delivered to `decide`, or a `KeyboardInterrupt` raised in Python while `sc-agent-py` is running.

**MUST statements:**

- Ctrl-C, SIGINT or SIGTERM received by `decide` MUST cancel the `CancelToken` of the running work.
- A `KeyboardInterrupt` in `sc-agent-py` MUST cancel the `CancelToken` of the running work.
- Every request not finished when the interrupt arrives MUST end with `decision.cancelled`.
- The session log MUST be flushed before `decide` exits or the Python call returns.
- `decide` MUST exit non-zero after an interrupt.
- Exception: a third interrupt received by `decide` after its logs have opened MUST end `decide` at once with exit 130, without waiting for the drain or the session-log flush. It is the operator's last resort when a drain cannot finish; requests not yet written may have no session row.

### Rationale

An interrupted batch must leave a complete, readable session log and a clear failure status instead of lost rows and a silent exit.

### Success Criteria

**Acceptance Criteria:**

- [ ] Sending SIGINT to `decide` during a batch makes every unfinished request end with `decision.cancelled`, flushes the session log, and exits non-zero.
- [ ] Sending SIGTERM to `decide` during a batch has the same result as SIGINT.
- [ ] A third interrupt to `decide` after its logs have opened exits 130 at once (the first and second only cancel).
- [ ] Raising `KeyboardInterrupt` during an `sc-agent-py` batch cancels the token, ends unfinished requests with `decision.cancelled`, flushes the session log, and re-raises `KeyboardInterrupt`.

### Dependencies

**Requires:**

- REQ-SAG-0104

**Related:**

- REQ-SAG-0701
- REQ-SAG-0005