# Requirement: Provider and model are separate

**Status:** Approved  
**Version:** 0.3.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0103: Provider and model are separate

**Status:** Approved  

### Requirement Statement

Definitions: A provider is the service a request is sent to, such as OpenRouter, Nous, Together, OpenAI or Anthropic. A model is what that provider runs.

**MUST statements:**

- Provider and model MUST be separate values; a short name MUST NOT encode its provider.
- Each model-table row MUST give the id its provider uses for that model (REQ-SAG-0102).
- sc-agent MUST be able to send requests through Together and OpenRouter as well as the direct providers.
- When no model entry names a provider, a short name MUST resolve to the first row of the model table, sorted newest first, whose provider is keyless or holds its credential (REQ-SAG-0102).
- A full model id MUST select the row with exactly that id and that row's provider.
- The provider for models served by the OpenAI API MUST be named `openai`.

**MUST NOT statements:**

- sc-agent MUST NOT require the caller to name a provider.

### Rationale

The owner uses OpenRouter, Nous, Together, OpenAI and Anthropic today, and one model can be served by more than one provider. Making the caller pick a provider per call is poor UX and unreliable over time.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

Lead ruling 2026-11-24 (phase j plan review): resolution follows REQ-SAG-0102 and its owner ruling of 2026-11-24 (the model table sorted newest first; the first row whose provider is keyless or holds a credential), replacing provider-table list order; a full id selects its row exactly.

### Success Criteria

**Acceptance Criteria:**

- [ ] The short name `haiku` selects the same model through two different providers, and each sends its own id: `claude-haiku-5-5` through `anthropic` and `anthropic/claude-haiku-5.5` through `openrouter`.
- [ ] With no model entry, a `haiku` request goes to the provider of the first `haiku` row in model-table order whose provider holds its credential.
- [ ] The full id `anthropic/claude-haiku-5.5` selects a row with exactly that id, and `claude-haiku-5-5` selects the `anthropic` row.
- [ ] The provider list contains `openai` for models served by the OpenAI API.