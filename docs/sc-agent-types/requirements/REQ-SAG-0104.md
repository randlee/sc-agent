# Requirement: Cancel token

**Status:** Approved  
**Version:** 0.3.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0104: Cancel token

**Status:** Approved  

### Requirement Statement

**MUST statements:**

- `sc-agent` MUST provide the cancel flag type, `Cancel`.
- Each Run MUST own one cancel flag, which the CLI's interrupt handler and the PyO3 `Run.cancel()` method cancel.

**MUST NOT statements:**

- The PyO3 bindings MUST NOT expose the cancel flag as its own Python object.

### Rationale

Streaming and launches in several crates must agree on one cancellation signal.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

Owner ruling 2026-11-24 (plan review, ADR-SAG-0003): the cancel flag belongs to the Run (ADR-SAG-0019 decision 14); Python cancels through `run.cancel()`.

### Success Criteria

**Acceptance Criteria:**

- [ ] A cancel flag created in one crate stops an operation in another crate when cancelled.
- [ ] `run.cancel()` in Python stops the Run's running batch, and `sc_agent` exports no `Cancel`.