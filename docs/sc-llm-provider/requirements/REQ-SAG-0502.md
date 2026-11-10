# Requirement: sc-agent can launch any LLM

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0502: sc-agent can launch any LLM

**Status:** Draft  

### Requirement Statement

**MUST statements:**

- sc-agent MUST be able to send a request to any LLM a configured provider serves, not only decision models.

### Rationale

Decision models are a small part of what sc-agent does.

### Success Criteria

**Acceptance Criteria:**

- [ ] sc-agent sends a request to a non-decision LLM selected by short name and returns its response.

### Dependencies

**Related:**

- REQ-SAG-0101
- REQ-SAG-0103