# Requirement: Python 3.11 is the floor for sc-agent's wheels and their consumers

**Status:** Proposed  
**Version:** 0.2.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-14  
**Owner:** Rand Lee  

---

## NFR-SAG-0003: Python 3.11 is the floor for sc-agent's wheels and their consumers

**Status:** Proposed  

### Requirement Statement

Definitions: The wheels are sc-decidr (`crates/sc-agent-py`) and sitr (`crates/sc-treesitter-py`). The abi3 floor is the `abi3-py3NN` feature of pyo3 that sets a wheel's stable-ABI tag.

**MUST statements:**

- The abi3 floor of `crates/sc-agent-py`, `crates/sc-treesitter-py` and `crates/sc-treesitter` (its optional pyo3) MUST be `abi3-py311`.
- Each wheel's `requires-python` MUST be `>=3.11`, and its wheel tag MUST be `cp311-abi3`.
- CI MUST build each wheel and run its pytest suite on Python 3.11.
- The consumer module rendered from the consumer template (REQ-SAG-0018) MUST declare `requires-python = ">=3.11"` in its script header.

**MUST NOT statements:**

- A wheel MUST NOT carry an abi3 tag below the `requires-python` floor.

### Rationale

`requires-python` already says `>=3.11` while pyo3 builds `cp310-abi3` wheels, so the tag claims support the package metadata refuses. One floor for sc-agent and every consumer removes that mismatch and lets consumers drop older CI floors.

### Success Criteria

**Acceptance Criteria:**

- [ ] The built sc-decidr and sitr wheels are tagged `cp311-abi3`.
- [ ] The CI wheel and pytest jobs for both wheels run on Python 3.11.
- [ ] A rendered consumer module's script header declares `requires-python = ">=3.11"`.

### Dependencies

**Related:**

- ADR-SAG-0019 decision 12
- REQ-SAG-0018
