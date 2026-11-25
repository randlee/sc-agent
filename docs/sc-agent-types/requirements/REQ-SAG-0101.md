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

- Each of the short names `jev`, `haiku`, `sonnet`, `opus`, `fable`, `luna`, `terra`, `sol`, `astra` MUST resolve to a model in the `sc-agent` provider tables.
- Each short name MUST select the latest model of its line.
- `jev` MUST select the latest Jev model.
- The model table MUST define the short names `laya` and `decider` for local models.
- A caller MUST be able to select a specific model version instead of a short name.

### Rationale

sc-agent launches any model, not only decision models, and callers select models by short name.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] Each of the nine short names resolves to the newest model of its line in the provider tables.
- [ ] `laya` and `decider` each resolve to a local model in the model table.
- [ ] A specific version id from the catalog parses and resolves to itself.

### Dependencies

**Related:**

- REQ-SAG-0102