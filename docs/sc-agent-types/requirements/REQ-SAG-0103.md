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

Definitions: A provider is the service a request is sent to, such as OpenRouter, Nous, Together, OpenAI, Anthropic or TypeSafe. A model is what that provider runs.

**MUST statements:**

- A caller MUST specify the provider and the model separately.
- OpenAI models MUST be named OpenAI models.

**MUST NOT statements:**

- `sc-agent-types` MUST NOT name OpenAI models "Codex models".

### Rationale

The owner uses OpenRouter, Nous, Together, OpenAI and Anthropic today, and one model can be served by more than one provider.

### Success Criteria

**Acceptance Criteria:**

- [ ] The same model id can be sent through two different providers without changing the model id.
- [ ] No public item in `sc-agent-types` is named for Codex.