# Requirement: Quota exhaustion is not busy

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0602: Quota exhaustion is not busy

**Status:** Draft  

### Requirement Statement

Definitions: Quota exhaustion is a response whose body says the account's quota, credit or spend limit is used up, in the documented Anthropic and OpenAI shapes.

**MUST statements:**

- A busy status whose body marks quota exhaustion MUST be returned at once as `QUOTA_EXHAUSTED`.
- A 429 with a body that does not mark quota exhaustion MUST be treated as busy.

**MUST NOT statements:**

- A quota-exhausted response MUST NOT be resent.

### Rationale

Resending cannot succeed until the account is topped up, so holding off only delays the error.

### Success Criteria

**Acceptance Criteria:**

- [ ] A 429 carrying the documented Anthropic quota-exhausted body returns `QUOTA_EXHAUSTED` after one send.
- [ ] A 429 with an unrecognized body is resent under REQ-SAG-0601.

### Dependencies

**Related:**

- REQ-SAG-0601