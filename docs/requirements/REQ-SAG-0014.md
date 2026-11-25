# Requirement: Input size limits

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0014: Input size limits

**Status:** Approved  

### Requirement Statement

**MUST statements:**

- Input too large to handle MUST be refused with an error before any send.

### Rationale

Bounded input keeps memory use predictable and turns an oversized input into a clear error before any send.

### Success Criteria

**Acceptance Criteria:**

- [ ] An input too large to handle is refused with an error, and no send is made.

### Dependencies

**Related:**

- REQ-SAG-0504
- REQ-SAG-0002