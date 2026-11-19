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

Definitions: The project/phase is the name of the project or phase a session belongs to, a block of time related to major changes. Its setting is named `project`. The layers are those of REQ-SAG-0019. A setting is resolved through a layer when that layer is looked up for the setting's value. The project layers are the layers of REQ-SAG-0019 that can set `project`, in this order: the `--project` flag, the `SC_PROJECT` environment variable, `project` in the local `<cli>.json`, `project` in the local `sc-agent.json`, `project` in the global `<cli>.json`, then `project` in the global `sc-agent.json`. The PyO3 bindings are the `sc_agent` Python module of `sc-agent-py`. The CLI text is every usage text, help text and error message. A typed error is an error detail (`ErrorDetail`, REQ-SAG-0016). A session write is a call that would write a session. A built-in default is the value that layer 7 of REQ-SAG-0019 gives a setting that no other layer sets. The setting places are `--project`, `SC_PROJECT`, `sc-agent.json` and `<cli>.json`. PyO3 is the Rust library `sc-agent-py` uses to build its Python module.

**MUST statements:**

- The `project` setting MUST be resolved through every project layer.
- The PyO3 bindings MUST take `project` from the first project layer that sets it.
- In the PyO3 bindings, a `project` argument MUST take the place of the `--project` flag layer.
- When no project layer sets `project`, a session write MUST fail with a typed error.
- When no project layer sets `project`, the typed error of a session write MUST name every setting place.
- The CLI text about the setting MUST call it "project/phase".

**MUST NOT statements:**

- When no project layer sets `project`, a session write MUST NOT send a session.
- When no project layer sets `project`, a session write MUST NOT write a session.
- When no project layer sets `project`, the built-in default layer, layer 7 of REQ-SAG-0019, MUST NOT give `project` a value.

### Rationale

Agents work by phase, so sessions are grouped by project/phase. A default would merge sessions of different phases into one file.

### Success Criteria

**Acceptance Criteria:**

- [ ] With `"project": "phase-c"` in the global `sc-agent.json` and nothing else, decidr and sitr sessions are filed under project `phase-c`.
- [ ] With `SC_PROJECT=phase-d` and `"project": "phase-c"` in the local `sc-agent.json`, sessions are filed under `phase-d`.
- [ ] With `--project phase-e`, sessions are filed under `phase-e` whatever the environment and files set.
- [ ] With no layer setting it, a decidr request fails with the typed error, nothing is sent, and no session row is written.
- [ ] With no layer setting it, the typed error of a failed decidr request names `--project`, `SC_PROJECT`, `sc-agent.json` and `<cli>.json`.
- [ ] With no layer setting it, a PyO3 call that would write a session fails with the same typed error.
- [ ] With `SC_PROJECT=phase-d`, a PyO3 call given the `project` argument `phase-f` files its session under `phase-f`.
- [ ] `decidr --help` describes `--project` as the project/phase.

### Dependencies

**Related:**

- REQ-SAG-0004
- REQ-SAG-0019
- REQ-SAG-0020
- ADR-SAG-0021
