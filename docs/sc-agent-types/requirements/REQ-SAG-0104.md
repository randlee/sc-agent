# Requirement: Cancel token

**Status:** Approved  
**Version:** 0.3.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-25  
**Owner:** Rand Lee  

---

## REQ-SAG-0104: Cancel token

**Status:** Approved  

### Requirement Statement

**MUST statements:**

- `sc-agent` MUST provide a `Cancel` type whose cancellation stops an operation running in another crate.
- The `sc-agent` command-line tool MUST stop a running operation when the user interrupts it.
- The PyO3 bindings MUST let Python code stop a running operation.

### Rationale

Streaming and launches in several crates must agree on one cancellation signal, so a cancel from the CLI or from Python reaches every crate doing the work.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] A unit test creates a `Cancel` in one crate, cancels it, and shows an operation in another crate stops.
- [ ] A CLI run interrupted by the user stops its running operation and exits.
- [ ] A Python test cancels a running operation through the bindings and shows it stops.