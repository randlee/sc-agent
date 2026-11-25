# Requirement: Local decision runners come from configuration

**Status:** Approved  
**Version:** 0.3.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0801: Local decision runners come from configuration

**Status:** Approved  

### Requirement Statement

Definitions: A local decision runner is a daemon on the user's machine or network that serves a decision model through the Jev API. A local endpoint is a keyless provider whose host and port come from configuration (REQ-SAG-0019). sc-agent has two local decision runners, each a model row on the local endpoint of the same name: `laya` and `decider`. A runner's model id is the `id` of its model entry `models.<name>`.

**MUST statements:**

- Each local decision runner's endpoint MUST be defined in configuration by host and port (`laya.host`, `laya.port`, `decider.host`, `decider.port`).
- Laya's model id MUST be set in configuration (`models.laya.id`); without it, a `laya` selection MUST return a missing-setting error.
- When `models.decider.id` is unset, a `decider` selection MUST succeed and its request body MUST carry no `model`.
- When `models.decider.id` is set, a `decider` request body MUST carry it as `model`.
- Each local decision runner MUST be selectable by its name wherever a decision model is selected.
- `sc-agent` MUST send requests to a local decision runner in the Jev API format.
- Adding a local decision runner MUST need only one provider constant, one model row and configuration.

**MUST NOT statements:**

- `sc-agent` MUST NOT hard-code the host or port of any runner.

### Rationale

People are building local runners that duplicate the Jev API, and they differ only in where they run and how they report their loaded models.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

Owner ruling 2026-11-24 (plan review, ADR-SAG-0003): laya and decider are models on configured local endpoints; decider's model id is optional and omitted from the body when unset.

### Success Criteria

**Acceptance Criteria:**

- [ ] A Laya host, port and model id set in configuration are used with no rebuild.
- [ ] With `decider.host` and `decider.port` set, `--model decider` sends a Jev API request with no `model` to that endpoint; with `models.decider.id` set the body carries that id.
- [ ] A `laya` selection without `models.laya.id` returns a missing-setting error naming every layer.
- [ ] A workspace search finds no host or port literal for a local runner outside tests.

### Dependencies

**Related:**

- ADR-SAG-0001
- REQ-SAG-0201