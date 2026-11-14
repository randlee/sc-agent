# Requirement: Error detail JSON written by 0.1.0 still reads in 0.2.0

**Status:** Proposed  
**Version:** 0.1.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-14  
**Owner:** Rand Lee  

---

## NFR-SAG-0002: Error detail JSON written by 0.1.0 still reads in 0.2.0

**Status:** Proposed  

### Requirement Statement

Definitions: 0.1.0-shaped detail JSON is an `ErrorDetail` serialized by sc-decidr 0.1.0. It has `id`, `message` and optionally `recovery` and `cause`. Session rows and stored result lines contain it.

**MUST statements:**

- 0.2.0 MUST deserialize 0.1.0-shaped detail JSON, at every cause level, with `status`, `attempts` and `retry_after_ms` unset.
- A detail with none of the new fields set MUST serialize to the same bytes 0.1.0 wrote.

### Rationale

Stored session logs and result files written by 0.1.0 must stay readable after the upgrade.

### Success Criteria

**Acceptance Criteria:**

- [ ] A checked-in 0.1.0 detail JSON with a two-level cause chain deserializes in 0.2.0, and every new field is `None`.
- [ ] Re-serializing it yields the original bytes.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0016
