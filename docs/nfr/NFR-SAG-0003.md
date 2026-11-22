# Requirement: One Python floor for sc-agent's wheel and its consumers

**Status:** Approved  
**Version:** 0.4.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## NFR-SAG-0003: One Python floor for sc-agent's wheel and its consumers

**Status:** Approved  

### Requirement Statement

Definitions:

- `floors.toml` is the file at the repository root that writes down the workspace's floor values for sc-compose templates to read.
- The Python floor is `[python].floor` in `floors.toml`.
- The abi3 feature is the `abi3-py3NN` feature of pyo3 that sets a wheel's stable-ABI tag.
- The Python binding crate is `crates/sc-agent-py`, which builds the sc-decidr wheel.

**MUST statements:**

- The sc-decidr wheel MUST declare Python 3.11 or newer.

### Rationale

The declared Python floor and the pyo3 abi3 tag disagreed, so the wheel tag claimed support the package metadata refused. One floor value, written down in `floors.toml`, removes that mismatch and makes raising the floor a one-line change. `floors.toml` records the value; nothing in sc-agent checks it.

Owner ruling (2026-11-21), Rand Lee: "My words are what I want, not excess control resulting." "No hand built checks."

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] The built sc-decidr wheel declares Python 3.11 or newer.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0018
