# Requirement: One Python floor for sc-agent's wheels and their consumers

**Status:** Proposed  
**Version:** 0.2.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-14  
**Owner:** Rand Lee  

---

## NFR-SAG-0003: One Python floor for sc-agent's wheels and their consumers

**Status:** Proposed  

### Requirement Statement

Definitions:

- `floors.toml` is the file at the repository root that writes down the workspace's floor values for sc-compose templates to read.
- The Python floor is `[python].floor` in `floors.toml`.
- The wheels are sc-decidr (`crates/sc-agent-py`) and sitr (`crates/sc-treesitter-py`).
- The abi3 feature is the `abi3-py3NN` feature of pyo3 that sets a wheel's stable-ABI tag. The abi3 tag of the Python floor is the `cp3NN-abi3` wheel tag whose `3NN` is the Python floor.
- The Python binding crates are `crates/sc-agent-py`, `crates/sc-treesitter-py` and `crates/sc-treesitter` (its optional pyo3).
- The consumer module is the Python file a consumer renders from the consumer template of REQ-SAG-0018.

**MUST statements:**

- The abi3 feature of every Python binding crate MUST match the Python floor.
- Each wheel's `requires-python` MUST declare the Python floor.
- The file name of each built wheel MUST carry the abi3 tag of the Python floor.
- The consumer template MUST read the Python floor from `floors.toml` for the consumer module's script header, which declares it as its `requires-python`.

**MUST NOT statements:**

- The file name of a built wheel MUST NOT carry an abi3 tag below the Python floor.

### Rationale

At the time of writing the Python floor is 3.11, while pyo3 builds `cp310-abi3` wheels, so the tag claims support the package metadata refuses. One floor value, written down in `floors.toml`, removes that mismatch, lets consumers drop older CI floors, and makes raising the floor a one-line change.

### Success Criteria

**Acceptance Criteria:**

- [ ] The built sc-decidr and sitr wheels carry the abi3 tag of the Python floor.
- [ ] A rendered consumer module's script header declares the Python floor as its `requires-python`.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0018
