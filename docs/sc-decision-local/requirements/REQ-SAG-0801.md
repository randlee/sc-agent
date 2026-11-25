# Requirement: Local decision runners come from configuration

**Status:** Approved  
**Version:** 0.7.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-25  
**Owner:** Rand Lee  

---

## REQ-SAG-0801: Local decision runners come from configuration

**Status:** Approved  

### Requirement Statement

Definitions: The Jev API is the request and reply format of TypeSafe's Jev decision service (ADR-SAG-0001). A local decision runner is a daemon on the user's machine or network that serves a decision model through the Jev API. sc-agent has two, `laya` and `decider`. A runner's network address is its host and port. A decision model is a model that `sc-agent` can send a decision request to. The model table is `sc-agent`'s list of decision models, one row per model id. The 4b decider model is the `decider-4b-v2.1` row. A local model is a model-table row whose model is served by a local decision runner. A paid API is a provider that needs an API key; its key is sent only to the provider's built-in endpoint.

**MUST statements:**

- Each local decision runner's network address MUST come from configuration.
- Each local decision runner's model MUST have a fixed model id in the model table.
- A selection of `decider` MUST resolve to the 4b decider model.
- Each local decision runner MUST be selectable by its name wherever a decision model is selected.
- `sc-agent` MUST send requests to a local decision runner in the Jev API format.
- Configuration MUST be able to add a local model that runs on another computer.

**MUST NOT statements:**

- `sc-agent` MUST NOT hard-code the network address of any local decision runner.
- Configuration MUST NOT change where a paid API's key is sent.

### Rationale

People are building local runners that duplicate the Jev API, and they differ only in where they run and how they report their loaded models. decider is an alternative local model to laya with a different connection string.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] A unit test sets `laya.host` and `laya.port` in configuration and checks the request goes to them with no rebuild.
- [ ] A unit test finds a fixed model id in the model-table row of every local decision runner model.
- [ ] A unit test resolves `decider` to the 4b decider row.
- [ ] A CLI run of each command that selects a decision model accepts `--model laya` and `--model decider`.
- [ ] A unit test with a stub local runner checks that the request it receives is in the Jev API format.
- [ ] A unit test sends to a local model row added in configuration with another computer's host and checks the request reaches that host.
- [ ] A search of the whole workspace, outside tests, finds no host or port literal for a local runner.
- [ ] A unit test with configuration naming another endpoint for a paid provider checks that the provider's key is still sent only to its built-in endpoint.

### Dependencies

**Related:**

- ADR-SAG-0001
- REQ-SAG-0201