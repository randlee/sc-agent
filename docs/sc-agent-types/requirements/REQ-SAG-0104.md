# Requirement: Cancel token

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0104: Cancel token

**Status:** Approved  

### Requirement Statement

**MUST statements:**

- `sc-agent` MUST provide the cancel flag type, `Cancel`.
- The cancel flag MUST be usable from the CLI and the PyO3 bindings.

### Rationale

Streaming and launches in several crates must agree on one cancellation signal.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] A cancel flag created in one crate stops an operation in another crate when cancelled.
- [ ] A cancel flag cancelled from Python stops the operation it was passed to.