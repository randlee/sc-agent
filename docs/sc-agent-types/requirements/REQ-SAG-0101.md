# Requirement: Model short names

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0101: Model short names

**Status:** Draft  

### Requirement Statement

Definitions: A short name is a model name without a version. A line is the series of releases of one model.

**MUST statements:**

- `sc-agent-types` MUST define the short names `jev`, `haiku`, `sonnet`, `opus`, `fable`, `luna`, `terra`, `sol` and `astra`.
- Each short name MUST select the latest model of its line.
- `jev` MUST select the latest Jev model.
- A caller MUST be able to select a specific model version instead of a short name.

### Rationale

sc-agent launches any model, not only decision models, and callers select models by short name.

### Success Criteria

**Acceptance Criteria:**

- [ ] Each of the nine short names parses and resolves to the newest catalog entry of its line.
- [ ] A specific version id from the catalog parses and resolves to itself.

### Dependencies

**Related:**

- REQ-SAG-0102