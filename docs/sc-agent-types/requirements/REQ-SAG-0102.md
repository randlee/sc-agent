# Requirement: Model catalog: static provider tables

**Status:** Approved  
**Version:** 0.4.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0102: Model catalog: static provider tables

**Status:** Approved  

### Requirement Statement

Definitions: The model catalog is the provider table and the model table built into `sc-agent`. A provider is an endpoint: a base address (a fixed remote URL, or a host and port from configuration), a wire and an optional credential-variable name. The model table has one row per model: short name, fixed model id, release date and the provider the row references. A provider serves a short name when the model table has a row for that short name referencing it. A model entry is the configuration object `models.<alias>` (REQ-SAG-0019). A credential source is one of the sources of REQ-SAG-0401, in its resolution order. A resolved model is the typed value a selector resolves to: alias, model id and provider.

**MUST statements:**

- Within one provider, a short name MUST resolve to that provider's row with that short name and the latest release date.
- When a model entry names a provider for an alias, the alias MUST resolve to that provider, with the entry's `id` when set.
- Otherwise, when a keyless provider serves the short name, the short name MUST resolve to the first such provider in provider-table order.
- Otherwise, sc-agent MUST take the credential sources in order and resolve the short name to the first provider, in provider-table order, whose credential the first source holding any serving provider's credential holds.
- A credential source that fails to read MUST count as not holding that credential.
- When no provider qualifies and a keyring read failed during resolution, resolution MUST return the keyring's error value.
- Otherwise, when no provider qualifies, resolution MUST return the error value `secrets.key_missing`, with category `auth`, naming every candidate's credential-variable name.
- Every model row, local endpoints included, MUST carry a fixed model id.
- The `nous` provider MUST serve `haiku` as `anthropic/claude-haiku-5.5` and `luna` as `openai/gpt-6-luna`.
- A model selector MUST be resolved once, at the CLI flag, at Run creation or at a per-call override, and the resolved model MUST be what the send, the cache key and the session row use.
- A full model id MUST select its row exactly.
- Each built-in model MUST carry the model's published release date.

**MUST NOT statements:**

- sc-agent MUST NOT branch on a provider's or a model's name.
- A model selector MUST NOT be resolved again after its resolved model is created.

### Rationale

When a new model is released, short names such as `haiku`, `luna`, `opus` and `sol` must launch it immediately.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

Owner ruling 2026-11-24 (plan review, ADR-SAG-0003): a provider is an endpoint, models are rows that reference provider constants, and there is no name-string branching. A model selection is resolved once into a typed value that carries the provider. Nous serves haiku and luna.

Owner ruling 2026-11-24 (supersedes the optional or configured local model id above): laya and decider have fixed model ids in the model table, like every other model. laya's row id is `laya:en`; decider has one fixed-id row per server (2b, 4b), and selecting `decider` resolves to its latest row, as for `haiku`. Whether a request body carries `model` is a property of the provider's wire, not of the model id. Cache keys always use the resolved fixed id.

Lead ruling 2026-11-24, pending owner confirmation: the resolution rule (a model entry wins, else the first provider whose credential resolves), and a credential source read error counts as not held. The alternative under owner review is provider-major: the first serving provider in table order that is keyless or whose credential resolves.

### Success Criteria

**Acceptance Criteria:**

- [ ] When two rows of one provider share a short name, the short name resolves to the row with the later release date.
- [ ] With only `ANTHROPIC_API_KEY` in the environment, `haiku` resolves to `anthropic`; with only `NOUS_API_KEY`, `haiku` resolves to `nous` with id `anthropic/claude-haiku-5.5` and `luna` to `nous` with id `openai/gpt-6-luna`, even though the nous rows tie with the first-table rows on release date.
- [ ] With `ANTHROPIC_API_KEY` in the environment and `models.haiku.provider` set to `openrouter`, `haiku` resolves to `openrouter`.
- [ ] With a key in the environment for a later provider and a key in `.env` for an earlier one, the environment's provider wins and the keyring is not read.
- [ ] With no credential for any provider that serves `haiku`, resolution returns `secrets.key_missing` with category `auth`, naming every candidate's credential-variable name.
- [ ] A search of `crates/sc-agent/src` finds no comparison of a provider or model name with a string literal outside the model and provider tables.

### Dependencies

**Related:**

- REQ-SAG-0101