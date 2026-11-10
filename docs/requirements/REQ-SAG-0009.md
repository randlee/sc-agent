# Requirement: Low-level crates work without tokio

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0009: Low-level crates work without tokio

**Status:** Approved  

### Requirement Statement

Definitions: tokio is the Rust async runtime. The low-level crates are `sc-agent-types`, `sc-agent-config`, `sc-agent-secrets`, `sc-decision-schema`, `sc-agent-http`, the decision backends, `sc-agent-log` and `sc-decision-runner`.

**MUST statements:**

- A program that does not use tokio MUST be able to use every low-level crate.

### Rationale

sc-agent may use tokio, but a CLI built on the low-level crates must work without it.

### Success Criteria

**Acceptance Criteria:**

- [ ] `cargo tree` for each low-level crate with default features lists no tokio crate.

### Dependencies

**Related:**

- ADR-SAG-0001
- ADR-SAG-0004