# Requirement: sc-treesitter is a stand-alone library

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0301: sc-treesitter is a stand-alone library

**Status:** Draft  

### Requirement Statement

**MUST statements:**

- `sc-treesitter` MUST be usable without any other sc-agent crate.

**MUST NOT statements:**

- `sc-treesitter` MUST NOT depend on `sc-agent-types` or any other sc-agent crate.
- `sc-treesitter` MUST NOT contain agent or decision-model code.

### Rationale

Source parsing is useful on its own and must not change when the agent crates change.

### Success Criteria

**Acceptance Criteria:**

- [ ] `cargo tree -p sc-treesitter` lists no sc-agent crate.