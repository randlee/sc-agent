# Requirement: Model short names

**Status:** Approved  
**Version:** 0.3.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0101: Model short names

**Status:** Approved  

### Requirement Statement

Definitions: A short name is a model name without a version. A line is the series of releases of one model.

**MUST statements:**

- The provider tables in `sc-agent` MUST define the short names `jev`, `haiku`, `sonnet`, `opus`, `fable`, `luna`, `terra`, `sol` and `astra`.
- Each short name MUST select the latest model of its line.
- `jev` MUST select the latest Jev model.
- The model table MUST define the local aliases `laya` and `decider`, each a row on the local endpoint of the same name (REQ-SAG-0801).
- A caller MUST be able to select a specific model version instead of a short name.

### Rationale

sc-agent launches any model, not only decision models, and callers select models by short name.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

Owner ruling 2026-11-24 (plan review, ADR-SAG-0003): laya and decider are models, rows on configured local endpoints; adding a model is one row plus configuration.

### Success Criteria

**Acceptance Criteria:**

- [ ] Each of the nine short names resolves to the newest model of its line in the provider tables.
- [ ] A specific version id from the catalog parses and resolves to itself.

### Dependencies

**Related:**

- REQ-SAG-0102