# Requirement: Model catalog: built-in models plus global configuration

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0102: Model catalog: built-in models plus global configuration

**Status:** Approved  

### Requirement Statement

Definitions: The model catalog is the union of the models built into sc-agent and the models read from the global configuration, each with its line and release date.

**MUST statements:**

- The catalog type in `sc-agent-types` MUST accept additional models read from the global configuration by a higher layer; `sc-agent-types` itself reads no files.
- A user MUST be able to add a model to the global configuration without upgrading any crate.
- A short name MUST resolve to the model of its line with the latest release date across the union of built-in and global-configuration models.
- Each built-in model MUST carry the model's published release date.

### Rationale

When a new model is released, short names such as `haiku`, `luna`, `opus` and `sol` must launch it immediately.

### Success Criteria

**Acceptance Criteria:**

- [ ] After a user adds to the global configuration a `haiku` model with a release date later than every built-in `haiku` model, `haiku` resolves to it with no rebuild.
- [ ] A user-added model with an earlier release date does not change what its short name resolves to.

### Dependencies

**Related:**

- REQ-SAG-0101