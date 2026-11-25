# Requirement: Local decision runners come from configuration

**Status:** Approved  
**Version:** 0.5.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0801: Local decision runners come from configuration

**Status:** Approved  

### Requirement Statement

Definitions: A local decision runner is a daemon on the user's machine or network that serves a decision model through the Jev API. A local endpoint is a keyless provider whose host and port come from configuration (REQ-SAG-0019). sc-agent has two local decision runners, `laya` and `decider`; each runner's models are fixed-id rows of the model table on local endpoints. The decider has two rows, `decider-2b-v11` and `decider-4b-v2.1`, each on its own local endpoint.

**MUST statements:**

- Each local endpoint MUST be defined in configuration by host and port (`<endpoint>.host`, `<endpoint>.port`).
- Each local decision runner's model MUST have a fixed model id in its model-table row: `laya:en` for laya, and `decider-2b-v11` and `decider-4b-v2.1` for the decider.
- A `decider` selection MUST resolve by the short-name rule of REQ-SAG-0102, as for any model.
- Every request to a local decision runner MUST carry the resolved model id as `model`.
- When a local decision runner's reply carries a `model` value that differs from the resolved row's id, the send MUST return the error value `decision.model_mismatch`, with category `response_invalid`, naming both ids.
- Each local decision runner MUST be selectable by its name wherever a decision model is selected.
- `sc-agent` MUST send requests to a local decision runner in the Jev API format.
- Adding a local decision runner MUST need only one provider constant, one model row and configuration.

**MUST NOT statements:**

- `sc-agent` MUST NOT hard-code the host or port of any runner.
- A send that returns `decision.model_mismatch` MUST NOT write a session row.

### Rationale

People are building local runners that duplicate the Jev API, and they differ only in where they run and how they report their loaded models.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

Owner ruling 2026-11-24 (plan review, ADR-SAG-0003): laya and decider are models on configured local endpoints; decider's model id is optional and omitted from the body when unset.

Owner ruling 2026-11-24 (supersedes the optional or configured local model id and the omitted `model` above): laya and decider have fixed model ids in the model table, like every other model. laya requires `laya:en`. decider has two rows, `decider-2b-v11` (port 18000) and `decider-4b-v2.1` (port 18001), both released 2026-09-24, and `decider` resolves by the same rule as `haiku`. The decider servers accept and ignore `model`, so every request carries the resolved id. Cache keys always use the resolved fixed id.

Owner ruling 2026-11-24 (configuration model rows): configuration can add rows for the local endpoints, and a local server that echoes its id (decider) must echo the resolved row's id; a mismatch is a typed error and writes no session row, so the cache never answers from it. laya already validates the requested id.

### Success Criteria

**Acceptance Criteria:**

- [ ] A laya host and port set in configuration are used with no rebuild, and a `laya` request carries `laya:en`.
- [ ] With both decider endpoints configured, `--model decider` resolves by the REQ-SAG-0102 rule and sends a Jev API request carrying that row's id as `model` to that row's endpoint; `--model decider-4b-v2.1` sends to the 4b endpoint; each session row's `model_requested` is the row's fixed id.
- [ ] A decider stub that replies with `"model": "decider-2b-v11"` to a `decider-4b-v2.1` request makes the send return `decision.model_mismatch` with category `response_invalid` naming both ids, and no session row is written.
- [ ] A local endpoint selection whose host or port is unset returns a missing-setting error naming every layer.
- [ ] A search of sc-agent's source (`crates/`, outside tests) finds no host or port literal for a local runner. Documentation of the owner's integration cases may name configured values.

### Dependencies

**Related:**

- ADR-SAG-0001
- REQ-SAG-0201