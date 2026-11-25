# Requirement: Invalid batch input fails fast

**Status:** Approved  
**Version:** 0.1.1  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0504: Invalid batch input fails fast

**Status:** Approved  

### Requirement Statement

Definitions: JSONL is newline-delimited JSON (one JSON object per line). API means application programming interface. A batch is the JSONL requests passed to `decidr` in one call. An invalid batch contains a request that does not match the Jev API request schema. The field is the field in that request that fails schema validation.

**MUST statements:**

- For an invalid batch, `decidr` MUST stop before sending any request.
- For an invalid batch, `decidr` MUST return an error.
- `decidr` MUST include the line in the invalid-batch error.
- `decidr` MUST include the field in the invalid-batch error.
- `decidr` MUST include the reason for the schema mismatch in the invalid-batch error.

### Rationale

A schema error in a large batch should be reported at once with enough detail to fix it, not after thousands of sends.

### Success Criteria

**Acceptance Criteria:**

- [ ] A unit test verifies that a 1,000-line batch whose line 412 has a `score` question without `criteria` returns an error naming line 412, `criteria` and the reason, and sends no request.

### Dependencies

**Related:**

- REQ-SAG-0201
- REQ-SAG-0701