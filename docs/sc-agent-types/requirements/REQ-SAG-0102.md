# Requirement: Model catalog: static provider tables

**Status:** Approved  
**Version:** 0.8.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-25  
**Owner:** Rand Lee  

---

## REQ-SAG-0102: Model catalog: static provider tables

**Status:** Approved  

### Requirement Statement

Definitions: The model catalog is the provider table and the model table built into `sc-agent`. A provider is an endpoint: a base address, a wire and an optional credential-variable name. A keyed provider is a provider with a credential-variable name; a keyless provider has none. The model table has one row per model: short name, fixed model id, release date and provider. A provider serves a short name when the model table has a row for that short name referencing it. A configuration row is a model row added by configuration; a built-in row is a row built into `sc-agent`. The merged table is the built-in rows together with the configuration rows. The configured provider is the provider a configuration file names for a model. A resolved model is the typed value a model selector resolves to: short name, fixed model id and provider.

**MUST statements:**

- The merged table MUST be sorted by release date, newest first.
- A short name MUST resolve to its newest row in the merged table.
- When several providers serve a short name, the configured provider MUST be selected.
- When several providers serve a short name and none is configured, a keyless provider MUST be selected over a keyed provider.
- When only keyed providers serve a short name, the first provider whose key is found MUST be selected.
- Every model row MUST carry a fixed model id.
- `decider` MUST resolve to the row `decider-4b-v2.1`.
- The `nous` provider MUST serve `haiku`.
- The `nous` provider MUST serve `luna`.
- A model selector MUST be resolved once, into a resolved model.
- After resolution, every use of the model MUST take the resolved model's fixed model id.
- A full model id MUST select its row exactly.
- Each built-in row MUST carry the model's published release date.

**MUST NOT statements:**

- sc-agent MUST NOT branch on a provider's or a model's name.
- A configuration row MUST NOT override a built-in row.
- Configuration MUST NOT change the base address of a keyed built-in provider.

### Rationale

When a new model is released, short names such as `haiku`, `luna`, `opus` and `sol` must launch it immediately. Hard-coded rows go stale quickly, so configuration can add rows, and a newer configuration row wins its short name. Configuration can add a local model that runs on another computer, but it cannot send a paid API's key to a different address. Provider and model are values, not names to branch on, so a model is resolved once and the send, the cache key and the session row all use the same fixed id. Local models such as `laya` and `decider` are named the way `haiku` is.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] A unit test verifies that the merged table, with a configuration row added, is sorted by release date, newest first.
- [ ] A unit test verifies that, of two rows sharing a short name, the short name resolves to the row with the later release date.
- [ ] A unit test verifies that, with keys for two providers serving `haiku`, the configured provider is selected.
- [ ] A unit test verifies that, with a keyless and a keyed provider serving a short name and none configured, the keyless provider is selected.
- [ ] A unit test verifies that, with two keyed providers serving `haiku` and only the second provider's key present, the second provider is selected.
- [ ] A unit test verifies that every row of the merged table has a non-empty model id.
- [ ] A unit test verifies that `decider` resolves to `decider-4b-v2.1`.
- [ ] A unit test verifies that, with only `NOUS_API_KEY` present, `haiku` resolves to the `nous` provider.
- [ ] A unit test verifies that, with only `NOUS_API_KEY` present, `luna` resolves to the `nous` provider.
- [ ] A unit test verifies that a send resolves its model selector exactly once.
- [ ] A unit test verifies that a send with a short name records the resolved fixed model id in its cache key.
- [ ] A unit test verifies that selecting a built-in row's full model id returns that row.
- [ ] A unit test verifies that every built-in row has a release date.
- [ ] A search of `crates/sc-agent/src` finds no comparison of a provider or model name with a string literal outside the model and provider tables.
- [ ] A unit test verifies that a configuration row whose `id` equals a built-in row's id fails the load with `config.invalid`.
- [ ] A unit test verifies that a configuration entry that changes the base address of a keyed built-in provider fails the load with `config.invalid`.

### Dependencies

**Related:**

- REQ-SAG-0101