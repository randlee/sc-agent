# Requirement: Model context limits

**Status:** Approved  
**Version:** 0.2.1  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0014: Model context limits

**Status:** Approved  

### Requirement Statement

**MUST statements:**

- A request that exceeds the target model’s context limit MUST be refused with an error before any send.

### Rationale

Context limits depend on the target model. Rejecting a request that exceeds that limit prevents a send that the model cannot handle.

### Success Criteria

**Acceptance Criteria:**

- [ ] A unit test verifies that a request exceeding the target model’s context limit returns an error and makes no send.

### Dependencies

**Related:**

- REQ-SAG-0504
- REQ-SAG-0002