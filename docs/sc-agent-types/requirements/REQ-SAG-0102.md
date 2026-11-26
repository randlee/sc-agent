# Requirement: Model catalog: built-in and configured model rows

**Status:** Approved  
**Version:** 0.9.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-26  
**Owner:** Rand Lee  

---

## REQ-SAG-0102: Model catalog: built-in and configured model rows

**Status:** Approved  

### Requirement Statement

Definitions: `sc-agent` is the program this record governs. The model table is the list of models built into `sc-agent`, together with the rows configuration adds. Each row has a short name, a fixed model id, a release date and a provider. A built-in model is a row built into `sc-agent`, not added by configuration. A full model id is a row's fixed model id, such as `claude-haiku-5-5`. A model's published release date is the date its maker announced the model as available. A catalog name is a provider name, a short name or a model id. To branch on a catalog name is to compare it with a string literal to choose what the code does.

**MUST statements:**

- The built-in rows of the model table MUST be sorted by release date, newest first.
- A short name MUST resolve to its newest row in the model table, unless a configured model entry for that name overrides it (REQ-SAG-0019).
- A full model id MUST select its row exactly.
- Each built-in model MUST carry the model's published release date.

**MUST NOT statements:**

- `sc-agent` MUST NOT branch on a catalog name.

### Rationale

When a new model is released, short names such as `haiku`, `luna`, `opus` and `sol` must launch it immediately. Hard-coded rows go stale quickly, so configuration can add rows, and a configured row wins its short name (owner ruling 2026-11-25). When several providers serve the newest row, REQ-SAG-0103 orders them. Provider and model are values carried in typed data, not names to branch on.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] A unit test verifies that, with a configuration row added, the built-in rows are sorted by release date, newest first, and the configuration row is selected for its short name.
- [ ] A unit test verifies that, of two rows sharing a short name, the short name resolves to the row with the later release date.
- [ ] A unit test verifies that selecting a built-in row's full model id returns that row.
- [ ] A unit test verifies that every built-in row has a release date.
- [ ] A search of `crates/sc-agent/src` finds no comparison of a catalog name with a string literal outside the model and provider tables.

### Dependencies

**Related:**

- REQ-SAG-0101
- REQ-SAG-0103