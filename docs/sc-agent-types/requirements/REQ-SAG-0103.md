# Requirement: Provider and model are separate

**Status:** Approved  
**Version:** 0.5.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-25  
**Owner:** Rand Lee  

---

## REQ-SAG-0103: Provider and model are separate

**Status:** Approved  

### Requirement Statement

Definitions: sc-agent is this project's client library and command-line tool that sends model requests. A provider is the service a request is sent to, such as OpenRouter, Nous, Together, OpenAI or Anthropic. A model is what that provider runs. A model id is the name a provider uses for a model, such as `anthropic/claude-haiku-5.5` at OpenRouter. A short name is the name a caller uses for a model, such as `haiku`. The model table is the list of models sc-agent can send to, one row per model and provider (REQ-SAG-0102). A key is the secret credential a provider requires before it accepts a request; a keyless provider requires none. The OpenAI API is the request service run by OpenAI.

**MUST statements:**

- A model-table row MUST store its provider in a field separate from its model.
- A request MUST carry the model id stored in the selected model-table row.
- sc-agent MUST be able to send requests through Together.
- sc-agent MUST be able to send requests through OpenRouter.
- When configuration names a provider for a model, sc-agent MUST send that model's requests to that provider.
- When no provider is configured for a model, sc-agent MUST prefer a provider that needs no key over one that needs a key.
- When no provider is configured for a model and every provider serving it needs a key, sc-agent MUST use the first of them whose key is found.
- The provider that sends requests to the OpenAI API MUST have the name `openai`.

**MUST NOT statements:**

- A short name MUST NOT contain a provider name.
- A request MUST NOT require the caller to name a provider.

### Rationale

The owner uses OpenRouter, Nous, Together, OpenAI and Anthropic today, and one model can be served by more than one provider. Making the caller pick a provider per call is poor UX and unreliable over time.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] Unit test: every model-table row stores its provider in a field separate from its model.
- [ ] Unit test: no short name in the model table contains a provider name.
- [ ] Unit test: the short name `haiku` selects the same model through two different providers, and each sends its own id: `claude-haiku-5-5` through `anthropic` and `anthropic/claude-haiku-5.5` through `openrouter`.
- [ ] Unit test: with `openrouter` configured for `haiku` and the `anthropic` key also found, a `haiku` request goes to `openrouter`.
- [ ] Unit test: with no provider configured, a model served by a keyless provider and a keyed provider goes to the keyless one.
- [ ] Unit test: with no provider configured and only the `openrouter` key found, a `haiku` request goes to `openrouter`.
- [ ] Unit test: requests can be sent through `together` and through `openrouter`.
- [ ] Unit test: a request that names only a model, with no provider, is sent.
- [ ] Unit test: the provider that sends to the OpenAI API is named `openai`.