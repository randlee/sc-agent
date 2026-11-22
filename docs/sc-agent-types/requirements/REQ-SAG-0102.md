# Requirement: Model catalog: static provider tables

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0102: Model catalog: static provider tables

**Status:** Approved  

### Requirement Statement

Definitions: The model catalog is the provider tables built into `sc-agent`. Each table lists its models as (short name, model id, release date).

**MUST statements:**

- A short name MUST resolve to the row with that short name and the latest release date.
- A full model id MUST select its row exactly.
- Each built-in model MUST carry the model's published release date.

### Rationale

When a new model is released, short names such as `haiku`, `luna`, `opus` and `sol` must launch it immediately.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] When two rows share a short name, the short name resolves to the row with the later release date.

### Dependencies

**Related:**

- REQ-SAG-0101