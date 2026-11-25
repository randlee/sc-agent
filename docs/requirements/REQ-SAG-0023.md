# Requirement: Run lifecycle audit events

**Status:** Draft
**Version:** 0.1.0
**Created:** 2026-11-24
**Last Updated:** 2026-11-24
**Owner:** Rand Lee

---

## REQ-SAG-0023: Run lifecycle audit events

**Status:** Draft

### Requirement Statement

Every tool MUST log each run’s start with its model and mode, and its end with pass or fail, an error code on failure, and elapsed time.

### Rationale

A common lifecycle makes tool runs comparable (D11).

### Success Criteria

**Acceptance Criteria:**

- [ ] Each tool records a run start containing model and mode.
- [ ] Each tool records a run end containing outcome, elapsed time and an error code on failure.

### Dependencies

**Related:**

- REQ-SAG-0018
- ADR-SAG-0019
- ADR-SAG-0021
