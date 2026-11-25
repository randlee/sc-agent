# Requirement: Local decision runners come from configuration

**Status:** Approved  
**Version:** 0.4.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0801: Local decision runners come from configuration

**Status:** Approved  

### Requirement Statement

Definitions: A local decision runner is a daemon on the user's machine or network that serves a decision model through the Jev API. A local endpoint is a keyless provider whose host and port come from configuration (REQ-SAG-0019). sc-agent has two local decision runners, `laya` and `decider`; each runner's models are fixed-id rows of the model table on local endpoints. The decider has one row per server (2b and 4b), each on its own local endpoint.

**MUST statements:**

- Each local endpoint MUST be defined in configuration by host and port (`<endpoint>.host`, `<endpoint>.port`).
- Each local decision runner's model MUST have a fixed model id in its model-table row; laya's is `laya:en`.
- A `decider` selection MUST resolve to the latest decider row, as a short name does for any model (REQ-SAG-0102).
- Whether a request body carries `model` MUST be decided by the provider's wire, not by the model id; the decider's wire sends no `model`.
- Each local decision runner MUST be selectable by its name wherever a decision model is selected.
- `sc-agent` MUST send requests to a local decision runner in the Jev API format.
- Adding a local decision runner MUST need only one provider constant, one model row and configuration.

**MUST NOT statements:**

- `sc-agent` MUST NOT hard-code the host or port of any runner.

### Rationale

People are building local runners that duplicate the Jev API, and they differ only in where they run and how they report their loaded models.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

Owner ruling 2026-11-24 (plan review, ADR-SAG-0003): laya and decider are models on configured local endpoints; decider's model id is optional and omitted from the body when unset.

Owner ruling 2026-11-24 (supersedes the optional or configured local model id above): laya and decider have fixed model ids in the model table, like every other model. laya's row id is `laya:en`; decider has one fixed-id row per server (2b, 4b), and selecting `decider` resolves to its latest row, as for `haiku`. Whether a request body carries `model` is a property of the provider's wire, not of the model id. Cache keys always use the resolved fixed id.

### Success Criteria

**Acceptance Criteria:**

- [ ] A laya host and port set in configuration are used with no rebuild, and a `laya` request carries `laya:en`.
- [ ] With both decider endpoints configured, `--model decider` resolves to the latest decider row, sends a Jev API request with no `model` to that row's endpoint, and its session row's `model_requested` is that row's fixed id.
- [ ] A local endpoint selection whose host or port is unset returns a missing-setting error naming every layer.
- [ ] A search of sc-agent's source (`crates/`, outside tests) finds no host or port literal for a local runner. Documentation of the owner's integration cases may name configured values.

### Dependencies

**Related:**

- ADR-SAG-0001
- REQ-SAG-0201