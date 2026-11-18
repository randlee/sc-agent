# Requirement: The project/phase setting is required and shared

**Status:** Proposed  
**Version:** 0.1.0  
**Created:** 2026-11-18  
**Last Updated:** 2026-11-18  
**Owner:** Rand Lee  

---

## REQ-SAG-0021: The project/phase setting is required and shared

**Status:** Proposed  

### Requirement Statement

Definitions: The project/phase is the name of the project or phase a session belongs to, a block of time related to major changes. Its setting is named `project`. The layers are those of REQ-SAG-0019.

**MUST statements:**

- The `project` setting MUST be resolved through every layer of REQ-SAG-0019, from the `--project` flag, the `SC_PROJECT` environment variable, `project` in `<cli>.json` and `project` in `sc-agent.json`.
- The PyO3 bindings MUST resolve `project` through the same layers, with a `project` argument as the flag layer.
- When no layer sets `project`, a call that would write a session MUST fail with a typed error that names `--project`, `SC_PROJECT`, `sc-agent.json` and `<cli>.json`, and no session MUST be sent or written.
- Every CLI usage text, help text and error message about the setting MUST call it "project/phase".

**MUST NOT statements:**

- `project` MUST NOT have a built-in default.

### Rationale

Agents work by phase, so sessions are grouped by project/phase. A default would merge sessions of different phases into one file.

### Success Criteria

**Acceptance Criteria:**

- [ ] With `"project": "phase-c"` in the global `sc-agent.json` and nothing else, decidr and sitr sessions are filed under project `phase-c`.
- [ ] With `SC_PROJECT=phase-d` and `"project": "phase-c"` in the local `sc-agent.json`, sessions are filed under `phase-d`.
- [ ] With `--project phase-e`, sessions are filed under `phase-e` whatever the environment and files set.
- [ ] With no layer setting it, a decidr request fails with the typed error, nothing is sent, and no session row is written.
- [ ] `decidr --help` describes `--project` as the project/phase.

### Dependencies

**Related:**

- REQ-SAG-0004
- REQ-SAG-0019
- REQ-SAG-0020
- ADR-SAG-0021
