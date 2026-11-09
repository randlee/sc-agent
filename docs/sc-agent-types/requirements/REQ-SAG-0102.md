# Requirement: User-editable model catalog

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0102: User-editable model catalog

**Status:** Draft  

### Requirement Statement

Definitions: The model catalog is the list of known models, each with its line and release date.

**MUST statements:**

- A user MUST be able to add a model to the catalog without upgrading any crate.
- A short name MUST resolve to the catalog model of its line with the latest release date, including models the user added.

### Rationale

When a new model is released, short names such as `haiku`, `luna`, `opus` and `sol` must launch it immediately.

### Success Criteria

**Acceptance Criteria:**

- [ ] After a user adds a `haiku` model with a release date later than every built-in `haiku` model, `haiku` resolves to it with no rebuild.
- [ ] A user-added model with an earlier release date does not change what its short name resolves to.

### Dependencies

**Related:**

- REQ-SAG-0101