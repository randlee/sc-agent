# Requirement: doctor warns about models missing from the catalog

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0503: doctor warns about models missing from the catalog

**Status:** Draft  

### Requirement Statement

Definitions: The catalog is the union of the models built into sc-agent and the models read from the global configuration. A provider's model list is what the provider's models endpoint returns.

**MUST statements:**

- `sc-ai doctor` MUST compare each configured provider's model list with the catalog.
- `sc-ai doctor` MUST warn about every model a provider lists that is neither built in nor in the global configuration, naming the provider and the model.

### Rationale

Short names run the latest model, so the catalog must keep up with what providers release.

### Success Criteria

**Acceptance Criteria:**

- [ ] With a provider listing a model that is in neither the built-in list nor the global configuration, `sc-ai doctor` prints a warning naming that provider and model.
- [ ] When every listed model is in the catalog, `sc-ai doctor` prints no missing-model warning.

### Dependencies

**Related:**

- REQ-SAG-0101
- REQ-SAG-0102