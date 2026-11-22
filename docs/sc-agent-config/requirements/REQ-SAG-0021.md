# Requirement: The project/phase setting is required and shared

**Status:** Proposed  
**Version:** 0.3.0  
**Created:** 2026-11-18  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0021: The project/phase setting is required and shared

**Status:** Proposed  

### Requirement Statement

Definitions: The project/phase is the name of the project or phase a session belongs to, a block of time related to major changes. Its setting is named `project`. The layers are those of REQ-SAG-0019. The project layers are the layers of REQ-SAG-0019 that can set `project`, in this order: the `--project` flag, the `SC_PROJECT` environment variable, `project` in the local `<cli>.json`, `project` in the local `sc-agent.json`, `project` in the global `<cli>.json`, then `project` in the global `sc-agent.json`. The PyO3 bindings are the `sc_agent` Python module of `sc-agent-py`. A typed error is an error value (REQ-SAG-0016). A session write is a call that would write a session. A built-in default is the value that layer 7 of REQ-SAG-0019 gives a setting that no other layer sets.

**MUST statements:**

- The `project` setting MUST be resolved through every project layer.
- The PyO3 bindings MUST take `project` from the first project layer that sets it.
- In the PyO3 bindings, a `project` argument MUST take the place of the `--project` flag layer.
- A session with no project/phase MUST be refused with an error.

**MUST NOT statements:**

- When no project layer sets `project`, a session write MUST NOT send a session.
- When no project layer sets `project`, a session write MUST NOT write a session.
- `project` MUST NOT have a built-in default.

### Rationale

Agents work by phase, so sessions are grouped by project/phase. A default would merge sessions of different phases into one file.

Owner ruling (2026-11-21), Rand Lee: "My words are what I want, not excess control resulting."

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] With `"project": "phase-c"` in the global `sc-agent.json` and nothing else, decidr sessions are filed under project `phase-c`.
- [ ] With `SC_PROJECT=phase-d` and `"project": "phase-c"` in the local `sc-agent.json`, sessions are filed under `phase-d`.
- [ ] With `--project phase-e`, sessions are filed under `phase-e` whatever the environment and files set.
- [ ] With no layer setting it, a decidr request fails with the typed error, nothing is sent, and no session row is written.

### Dependencies

**Related:**

- REQ-SAG-0004
- REQ-SAG-0019
- REQ-SAG-0020
- ADR-SAG-0021
