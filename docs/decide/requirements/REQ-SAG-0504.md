# Requirement: Invalid batch input fails fast

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0504: Invalid batch input fails fast

**Status:** Approved  

### Requirement Statement

Definitions: A batch is the JSONL requests passed to `decide` in one call.

**MUST statements:**

- When a request in a batch does not match the Jev API request schema, `decide` MUST stop before sending any request and return an error naming the line, the field and what was wrong.

### Rationale

A schema error in a large batch should be reported at once with enough detail to fix it, not after thousands of sends.

### Success Criteria

**Acceptance Criteria:**

- [ ] A 1,000-line batch whose line 412 has a `score` question without `criteria` returns an error naming line 412, `criteria` and the reason, and sends no request.

### Dependencies

**Related:**

- REQ-SAG-0201
- REQ-SAG-0701