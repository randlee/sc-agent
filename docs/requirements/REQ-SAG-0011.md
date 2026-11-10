# Requirement: Interrupt cancels cleanly

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

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

### Rationale

An interrupted batch must leave a complete, readable session log and a clear failure status instead of lost rows and a silent exit.

### Success Criteria

**Acceptance Criteria:**

- [ ] Sending SIGINT to `decide` during a batch makes every unfinished request end with `decision.cancelled`, flushes the session log, and exits non-zero.
- [ ] Sending SIGTERM to `decide` during a batch has the same result as SIGINT.
- [ ] Raising `KeyboardInterrupt` during an `sc-agent-py` batch cancels the token, ends unfinished requests with `decision.cancelled`, flushes the session log, and re-raises `KeyboardInterrupt`.

### Dependencies

**Requires:**

- REQ-SAG-0104

**Related:**

- REQ-SAG-0701
- REQ-SAG-0005