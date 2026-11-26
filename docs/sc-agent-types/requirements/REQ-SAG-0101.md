# Requirement: Model short names

**Status:** Approved  
**Version:** 0.4.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-26  
**Owner:** Rand Lee  

---

## REQ-SAG-0101: Model short names

**Status:** Approved  

### Requirement Statement

Definitions: A short name is a model name without a version. A line is the series of releases of one model. The cloud short names are `jev`, `haiku`, `sonnet`, `opus`, `fable`, `luna`, `terra`, `sol` and `astra`. The local short names are `laya` and `decider`. The model table (the provider tables) is the list of model rows in `sc-agent`; each row maps a model id to a provider. A local model is a row whose provider is an endpoint on a computer the user runs. A specific model version is a model id that includes its version.

**MUST statements:**

- Each cloud short name MUST resolve to a model in the `sc-agent` provider tables.
- Each short name MUST select the latest model of its line, unless configuration sets another default for that short name.
- `jev` MUST select the newest Jev row in the model table, unless a config file sets another default for `jev`, so callers get the latest Jev without a code change (ANC-001; Rand Lee 2026-11-26: "I want 'jev' to select latest, config can override default.").
- Each local short name MUST resolve to a local model in the model table.
- A caller MUST be able to select a specific model version instead of a short name.

### Rationale

sc-agent launches any model, not only decision models, and callers select models by short name.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] With no configured default, each of the nine short names resolves to the newest model of its line in the provider tables.
- [ ] A unit test resolves each cloud short name to a model in the provider tables.
- [ ] A unit test resolves `jev` to the newest Jev row, and, with a config default for `jev` set, to the configured model id.
- [ ] A unit test resolves each local short name to a local model in the model table.
- [ ] A specific version id from the catalog parses and resolves to itself.

### Dependencies

**Related:**

- REQ-SAG-0102