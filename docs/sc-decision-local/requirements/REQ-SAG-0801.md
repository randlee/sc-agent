# Requirement: Local decision runners come from configuration

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0801: Local decision runners come from configuration

**Status:** Approved  

### Requirement Statement

Definitions: A local decision runner is a daemon on the user's machine or network that serves a decision model through the Jev API. sc-agent has one, Laya, the `local` provider table's `laya` model.

**MUST statements:**

- The local decision runner MUST be defined in configuration by host, port and model (`laya.host`, `laya.port`, `laya.model`).
- The local decision runner MUST be selectable by the name `laya` wherever a decision model is selected.
- `sc-agent` MUST send requests to the local decision runner in the Jev API format.

**MUST NOT statements:**

- `sc-agent` MUST NOT hard-code the host or port of any runner.

### Rationale

People are building local runners that duplicate the Jev API, and they differ only in where they run and how they report their loaded models.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] A Laya host, port and model set in configuration are used with no rebuild.
- [ ] A workspace search finds no host or port literal for a local runner outside tests.

### Dependencies

**Related:**

- ADR-SAG-0001
- REQ-SAG-0201