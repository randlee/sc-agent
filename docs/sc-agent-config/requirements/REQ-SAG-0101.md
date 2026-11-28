# Requirement: Model short names

**Status:** Approved  
**Version:** 0.5.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-26  
**Owner:** Rand Lee  

---

## REQ-SAG-0101: Model short names

**Status:** Approved  

### Requirement Statement

Definitions: A short name is a model name without a version. The cloud short names are `jev`, `haiku`, `sonnet`, `opus`, `fable`, `luna`, `terra`, `sol` and `astra`. The local short names are `laya` and `decider`. A local model is a row whose provider is an endpoint on a computer the user runs.

**MUST statements:**

- Each cloud short name MUST resolve to a model in the `sc-agent` model table.
- Each local short name MUST resolve to a local model in the model table.

### Rationale

sc-agent launches any model, not only decision models, and callers select models by short name.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] A unit test resolves each cloud short name to a model in the model table.
- [ ] A unit test resolves each local short name to a local model in the model table.

### Dependencies

**Related:**

- REQ-SAG-0102