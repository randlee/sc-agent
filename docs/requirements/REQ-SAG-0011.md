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

Definitions: A Run is the context object through which every query is sent; it owns the cancel flag (ADR-SAG-0019 decision 14). An interrupt is Ctrl-C, SIGINT or SIGTERM delivered to `decidr`, or a `KeyboardInterrupt` raised in Python while `sc-agent-py` is running.

**MUST statements:**

- Ctrl-C, SIGINT or SIGTERM received by `decidr` MUST cancel the running Run's cancel flag.
- A `KeyboardInterrupt` in `sc-agent-py` MUST cancel the Run whose call is running.
- Every request not finished when the interrupt arrives MUST end with `decision.cancelled`.
- The session log MUST be flushed before `decidr` exits or the Python call returns.
- A `KeyboardInterrupt` in `sc-agent-py` MUST be caught; the Python call MUST return its results, with every unfinished request as a `decision.cancelled` error value.
- `decidr` MUST exit non-zero after an interrupt.

### Rationale

An interrupted batch must leave a complete, readable session log and a clear failure status instead of lost rows and a silent exit.

Owner ruling (2026-11-20), Rand Lee: "Python exceptions need to be caught (return error) so app doesn't crash."

Owner ruling (2026-11-21), Rand Lee: "always return descriminated unions w/ error info"

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

Owner ruling 2026-11-24 (plan review, ADR-SAG-0003): the cancel flag belongs to the Run; an interrupt cancels the Run, and Python has no separate cancel object.

### Success Criteria

**Acceptance Criteria:**

- [ ] Sending SIGINT to `decidr` during a batch makes every unfinished request end with `decision.cancelled`, flushes the session log, and exits non-zero.
- [ ] Sending SIGTERM to `decidr` during a batch has the same result as SIGINT.
- [ ] Raising `KeyboardInterrupt` during an `sc-agent-py` batch cancels the Run, ends unfinished requests with `decision.cancelled`, flushes the session log, and returns the results without raising.

### Dependencies

**Requires:**

- REQ-SAG-0104

**Related:**

- REQ-SAG-0701
- REQ-SAG-0005