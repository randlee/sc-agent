# Requirement: One Python floor for sc-agent's wheels and their consumers

**Status:** Approved  
**Version:** 0.3.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## NFR-SAG-0003: One Python floor for sc-agent's wheels and their consumers

**Status:** Approved  

### Requirement Statement

Definitions:

- `floors.toml` is the file at the repository root that writes down the workspace's floor values for sc-compose templates to read.
- The Python floor is `[python].floor` in `floors.toml`.
- The abi3 feature is the `abi3-py3NN` feature of pyo3 that sets a wheel's stable-ABI tag.
- The Python binding crates are `crates/sc-agent-py` and `crates/sc-treesitter` (its optional pyo3).

**MUST statements:**

- The wheels MUST declare Python 3.11 or newer.

### Rationale

The declared Python floor and the pyo3 abi3 tag disagreed, so the wheel tag claimed support the package metadata refused. One floor value, written down in `floors.toml`, removes that mismatch and makes raising the floor a one-line change. `floors.toml` records the value; nothing in sc-agent checks it.

Owner ruling (2026-11-21), Rand Lee: "My words are what I want, not excess control resulting." "No hand built checks."

### Success Criteria

**Acceptance Criteria:**

- [ ] The built wheels declare Python 3.11 or newer.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0018
