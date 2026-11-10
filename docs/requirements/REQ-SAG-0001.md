# Requirement: Created ids are ULIDs

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0001: Created ids are ULIDs

**Status:** Approved  

### Requirement Statement

Definitions: An id is any identifier sc-agent generates, such as a session id or a send id. ULID is the 26-character Crockford base32 Universally Unique Lexicographically Sortable Identifier.

**MUST statements:**

- Every id sc-agent generates MUST be a ULID.
- An id whose format an external API defines MUST use that API's format, including UUID where the API defines a UUID.

**MUST NOT statements:**

- sc-agent MUST NOT generate a UUID for an id that no external API defines.

### Rationale

One sortable id format across all session logs lets rows from every crate and machine be ordered and joined without conversion.

### Success Criteria

**Acceptance Criteria:**

- [ ] Every id written to a session log by sc-agent parses as a ULID.
- [ ] A workspace search finds UUID generation only at call sites that build a request for an external API that defines a UUID.

### Product Applicability

**Applies To:**

- All sc-agent crates, the CLI and the PyO3 bindings.