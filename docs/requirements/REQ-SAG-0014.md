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

Definitions: The target model is the model selected to receive the request.

**MUST statements:**

- When a request exceeds the target model’s context limit, sc-agent MUST return an error.

**MUST NOT statements:**

- sc-agent MUST NOT send a request that exceeds the target model’s context limit.

### Rationale

Context limits depend on the target model. Rejecting a request that exceeds that limit prevents a send that the model cannot handle.

### Success Criteria

**Acceptance Criteria:**

- [ ] A unit test verifies that a request exceeding the target model’s context limit returns an error and makes no send.

### Dependencies

**Related:**

- REQ-SAG-0504
- REQ-SAG-0002