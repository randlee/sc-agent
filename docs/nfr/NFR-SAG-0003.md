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

Definitions:

- The wheels are sc-decidr (`crates/sc-agent-py`) and sitr (`crates/sc-treesitter-py`).
- The abi3 floor is the `abi3-py3NN` feature of pyo3 that sets a wheel's stable-ABI tag.
- The Python binding crates are `crates/sc-agent-py`, `crates/sc-treesitter-py` and `crates/sc-treesitter` (its optional pyo3).
- CI (continuous integration) is the GitHub Actions workflow `.github/workflows/ci.yml`.
- A wheel job is a CI job that builds one wheel and runs that wheel's pytest suite.
- The consumer module is the Python file a consumer renders from the consumer template of REQ-SAG-0018.

**MUST statements:**

- The abi3 floor of every Python binding crate MUST be `abi3-py311`.
- Each wheel's `requires-python` MUST be `>=3.11`.
- The file name of each built wheel MUST carry the tag `cp311-abi3`.
- CI MUST run every wheel job on Python 3.11.
- The consumer module MUST declare `requires-python = ">=3.11"` in its script header.

**MUST NOT statements:**

- The file name of a built wheel MUST NOT carry an abi3 tag below the `requires-python` floor.

### Rationale

`requires-python` already says `>=3.11` while pyo3 builds `cp310-abi3` wheels, so the tag claims support the package metadata refuses. One floor for sc-agent and every consumer removes that mismatch and lets consumers drop older CI floors.

### Success Criteria

**Acceptance Criteria:**

- [ ] The built sc-decidr and sitr wheels are tagged `cp311-abi3`.
- [ ] The CI wheel and pytest jobs for both wheels run on Python 3.11.
- [ ] A rendered consumer module's script header declares `requires-python = ">=3.11"`.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0018
