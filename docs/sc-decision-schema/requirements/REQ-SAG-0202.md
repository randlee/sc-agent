# Requirement: Request digest for cache lookup

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0202: Request digest for cache lookup

**Status:** Draft  

### Requirement Statement

Definitions: `request_sha256` is the SHA-256 digest of the complete request body sent to the decision API.

**MUST statements:**

- Every decision send MUST record `request_sha256` computed over 100% of the request body sent.
- `request_sha256` MUST be the key for cached-response lookup.

### Rationale

Two sends with the same digest sent identical requests, so a stored answer can stand in for a repeat. The digest is a decision-model concern, so it belongs in `sc-decision-schema`, not `sc-agent-types`.

### Success Criteria

**Acceptance Criteria:**

- [ ] Two sends of byte-identical request bodies record the same `request_sha256`.
- [ ] Changing any byte of a request body changes its `request_sha256`.
- [ ] A lookup by `request_sha256` returns the stored answer of a previous successful send with that digest.

### Dependencies

**Related:**

- REQ-SAG-0004