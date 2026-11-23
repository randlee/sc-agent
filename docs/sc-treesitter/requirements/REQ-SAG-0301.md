# Requirement: sc-treesitter holds no agent code

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0301: sc-treesitter holds no agent code

**Status:** Approved  

### Requirement Statement

**MUST NOT statements:**

- `sc-treesitter` MUST NOT depend on any sc-agent crate other than `sc-agent`, whose error and logging it shares.
- `sc-treesitter` MUST NOT contain agent or decision-model code.

### Rationale

Source parsing is useful on its own and must not change when the agent crates change.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] `cargo tree -p sc-treesitter` lists no sc-agent crate other than `sc-agent`.