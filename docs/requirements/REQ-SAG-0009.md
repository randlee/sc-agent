# Requirement: Low-level crates work without tokio

**Status:** Approved  
**Version:** 0.2.1  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0009: Low-level crates work without tokio

**Status:** Approved  

### Requirement Statement

Definitions: tokio is the Rust async runtime. The low-level crates are `sc-agent` and `sc-treesitter`.

**MUST statements:**

- A program that does not use tokio MUST be able to use every low-level crate.

### Rationale

sc-agent may use tokio, but a CLI built on the low-level crates must work without it.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] A CLI run of `cargo tree` for each low-level crate with default features lists no tokio crate.

### Dependencies

**Related:**

- ADR-SAG-0001
- ADR-SAG-0004