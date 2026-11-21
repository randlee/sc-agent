# Requirement: doctor warns about models missing from the catalog

**Status:** Approved  
**Version:** 0.3.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0503: doctor warns about models missing from the catalog

**Status:** Approved  

### Requirement Statement

Definitions: The catalog is the union of the models built into sc-agent and the models read from the global configuration. A provider's model list is what the provider's models endpoint returns.

**MUST statements:**

- `decidr doctor` MUST compare each configured provider's model list with the catalog.
- `decidr doctor` MUST warn about every model a provider lists that is neither built in nor in the global configuration, naming the provider and the model.
- `decidr doctor` MUST list every detected provider key in the order sc-agent would use it.
- For each detected key, `decidr doctor` MUST show the source it came from: the environment, the repository-root `.env`, or the platform keyring.

### Rationale

Short names run the latest model, so the catalog must keep up with what providers release.

Owner ruling (2026-11-20), Rand Lee, on showing where a key came from: "I think this will help if there is key issue."

Owner ruling (2026-11-21), Rand Lee: "My words are what I want, not excess control resulting." "No hand built checks."

### Success Criteria

**Acceptance Criteria:**

- [ ] With a provider listing a model that is in neither the built-in list nor the global configuration, `decidr doctor` prints a warning naming that provider and model.
- [ ] When every listed model is in the catalog, `decidr doctor` prints no missing-model warning.
- [ ] With Anthropic and OpenRouter keys both set, `decidr doctor` lists both in priority order.

### Dependencies

**Related:**

- REQ-SAG-0101
- REQ-SAG-0102