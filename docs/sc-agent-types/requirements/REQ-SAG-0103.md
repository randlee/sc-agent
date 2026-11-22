# Requirement: Provider and model are separate

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0103: Provider and model are separate

**Status:** Approved  

### Requirement Statement

Definitions: A provider is the service a request is sent to, such as OpenRouter, Nous, Together, OpenAI or Anthropic. A model is what that provider runs.

**MUST statements:**

- Provider and model MUST be separate values; a catalog model id MUST NOT encode its provider.
- Each provider MUST map a catalog model id to the id that provider uses for that model.
- sc-agent MUST be able to send requests through Together and OpenRouter as well as the direct providers.
- When no provider is named, sc-agent MUST use the first provider table, in list order, that has the selected model.
- The provider for models served by the OpenAI API MUST be named `openai`.

**MUST NOT statements:**

- sc-agent MUST NOT require the caller to name a provider.

### Rationale

The owner uses OpenRouter, Nous, Together, OpenAI and Anthropic today, and one model can be served by more than one provider. Making the caller pick a provider per call is poor UX and unreliable over time.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] One catalog model id selects the same model through two different providers; each provider sends the id that provider uses for it (for example OpenRouter's `anthropic/claude-haiku-5.5` for `claude-haiku-5-5`).
- [ ] With no provider argument, a `haiku` request goes to the first provider table in list order that has `haiku`.
- [ ] The provider list contains `openai` for models served by the OpenAI API.