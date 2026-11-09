# Requirement: Provider and model are separate

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0103: Provider and model are separate

**Status:** Draft  

### Requirement Statement

Definitions: A provider is the service a request is sent to, such as OpenRouter, Nous, Together, OpenAI or Anthropic. A model is what that provider runs.

**MUST statements:**

- A caller MUST specify the provider and the model separately.
- OpenAI models MUST be named OpenAI models.

### Rationale

The owner uses OpenRouter, Nous, Together, OpenAI and Anthropic today, and one model can be served by more than one provider.

### Success Criteria

**Acceptance Criteria:**

- [ ] The same model id can be sent through two different providers without changing the model id.