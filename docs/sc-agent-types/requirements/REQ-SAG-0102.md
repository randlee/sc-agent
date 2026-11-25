# Requirement: Model catalog: static provider tables

**Status:** Approved  
**Version:** 0.7.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0102: Model catalog: static provider tables

**Status:** Approved  

### Requirement Statement

Definitions: The model catalog is the provider table and the model table built into `sc-agent`. A provider is an endpoint: a base address (a fixed remote URL, or a host and port from configuration), a wire and an optional credential-variable name. The model table has one row per model: short name, fixed model id, release date and the provider the row references. A provider serves a short name when the model table has a row for that short name referencing it. A model entry is the configuration object `models.<alias>` (REQ-SAG-0019); it adds one row to the model table. A configuration row is a row a model entry adds; a built-in row is a row of the table built into `sc-agent`. The merged table is the built-in rows together with the configuration rows. A credential source is one of the sources of REQ-SAG-0401, in its resolution order. A resolved model is the typed value a selector resolves to: alias, model id and provider.

**MUST statements:**

- The merged table MUST be sorted by release date, newest first; rows that share a release date keep their table order, built-in rows before configuration rows.
- A configuration row whose `id` equals a built-in row's id MUST be refused with the error value `config.invalid`, naming the file and key path (REQ-SAG-0019).
- A short name MUST resolve to the first row, in merged-table order, that has that short name and whose provider is keyless or has its credential in one of the credential sources.
- A credential source that fails to read MUST count as not holding that credential.
- When no provider qualifies and a keyring read failed during resolution, resolution MUST return the keyring's error value.
- Otherwise, when no provider qualifies, resolution MUST return the error value `secrets.key_missing`, with category `auth`, naming every candidate's credential-variable name.
- Every model row, local endpoints included, MUST carry a fixed model id.
- The `nous` provider MUST serve `haiku` as `anthropic/claude-haiku-5.5` and `luna` as `openai/gpt-6-luna`.
- A model selector MUST be resolved once, at the CLI flag or at Run creation, and the resolved model MUST be what the send, the cache key and the session row use; the cache key uses the resolved row's fixed id.
- A full model id MUST select its row exactly.
- Each built-in model MUST carry the model's published release date.

**MUST NOT statements:**

- sc-agent MUST NOT branch on a provider's or a model's name.
- A model selector MUST NOT be resolved again after its resolved model is created.
- A configuration row MUST NOT change or remove a built-in row.

### Rationale

When a new model is released, short names such as `haiku`, `luna`, `opus` and `sol` must launch it immediately.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

Owner ruling 2026-11-24 (plan review, ADR-SAG-0003): a provider is an endpoint, models are rows that reference provider constants, and there is no name-string branching. A model selection is resolved once into a typed value that carries the provider. Nous serves haiku and luna.

Owner ruling 2026-11-24 (supersedes the optional or configured local model id and the omitted `model` above): laya and decider have fixed model ids in the model table, like every other model. laya requires `laya:en`. decider has two rows, `decider-2b-v11` (port 18000) and `decider-4b-v2.1` (port 18001), both released 2026-09-24, and `decider` resolves by the same rule as `haiku`. The decider servers accept and ignore `model`, so every request carries the resolved id. Cache keys always use the resolved fixed id.

Owner ruling 2026-11-24 (plan review), on resolving a short name to the first row in table order whose provider is keyless or holds a credential: "that is fine as long as the table is sorted (newest-oldest) and verified to be sorted in unit test." A credential source read error counts as not held.

Lead ruling 2026-11-24 (phase j plan review): a model entry that names a provider without a row for its alias needs an `id`; without one the alias is unknown for that provider.

Owner ruling 2026-11-24 (configuration model rows; supersedes the configured-provider rule and the lead ruling on a model entry's `id` above): configuration can add model rows, because hard-coded rows go stale. Configuration rows merge with the built-in table, the merged table is sorted newest first and the sort unit test covers it, and short-name resolution runs over the merged table, so a configuration row with a newer `released` wins its short name. Built-in rows cannot be edited or removed.

Owner ruling 2026-11-24: `decider` defaults to 4b. The two decider rows share a release date, so `decider-4b-v2.1` is listed before `decider-2b-v11` and `decider` resolves to it.

### Success Criteria

**Acceptance Criteria:**

- [ ] A unit test verifies that the merged table, with a configuration row added, is sorted by release date, newest first.
- [ ] When two rows of one provider share a short name, the short name resolves to the row with the later release date.
- [ ] With only `ANTHROPIC_API_KEY` in the environment, `haiku` resolves to `anthropic`; with only `NOUS_API_KEY`, `haiku` resolves to `nous` with id `anthropic/claude-haiku-5.5` and `luna` to `nous` with id `openai/gpt-6-luna`, even though the nous rows tie with the first-table rows on release date.
- [ ] With `ANTHROPIC_API_KEY` in the environment and a `haiku` configuration row on `anthropic` with id `claude-haiku-5-6` and a `released` later than every built-in `haiku` row, `haiku` resolves to `anthropic` with id `claude-haiku-5-6`.
- [ ] With keys for two providers that serve `haiku`, the provider of the earlier row in table order wins wherever each key is held.
- [ ] With both decider endpoints configured, `decider` resolves to the first decider row in table order.
- [ ] With both decider endpoints configured and a `decider` configuration row on a decider endpoint whose `released` is later than 2026-09-24, `decider` resolves to that row's id; a configuration row whose `id` is `decider-4b-v2.1` fails the load with `config.invalid` naming the file and `models.decider.id`.
- [ ] With no credential for any provider that serves `haiku`, resolution returns `secrets.key_missing` with category `auth`, naming every candidate's credential-variable name.
- [ ] A search of `crates/sc-agent/src` finds no comparison of a provider or model name with a string literal outside the model and provider tables.

### Dependencies

**Related:**

- REQ-SAG-0101