# Requirement: sc-agent can launch any LLM

**Status:** Approved  
**Version:** 0.1.1  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0502: sc-agent can launch any LLM

**Status:** Approved  

### Requirement Statement

Definitions: LLM means large language model. A provider is the service a request is sent to, such as OpenRouter, Nous, Together, OpenAI or Anthropic. A decision model is a model used for decision requests, such as Jev. sc-agent is the product provided by this repository.

**MUST statements:**

- sc-agent MUST be able to send a request to any LLM a configured provider serves, not only decision models.

### Rationale

Decision models are a small part of what sc-agent does.

### Success Criteria

**Acceptance Criteria:**

- [ ] A unit test verifies that sc-agent sends a request to a non-decision LLM selected by short name and returns its response.

### Dependencies

**Related:**

- REQ-SAG-0101
- REQ-SAG-0103