# Requirement: Configuration environment variable names

**Status:** Proposed  
**Version:** 0.2.0  
**Created:** 2026-11-18  
**Last Updated:** 2026-11-20  
**Owner:** Rand Lee  

---

## REQ-SAG-0020: Configuration environment variable names

**Status:** Proposed  

### Requirement Statement

Definitions: A CLI is one of sc-agent's command-line programs, such as `decidr` or `sitr`. `<CLI>` is the CLI's program name in upper case with each `-` replaced by `_`, such as `DECIDR` or `SITR`. A key path is the list of key names from the top of a configuration file to a setting. `<KEY>` is a key name in upper case. A shared setting is a setting that applies to every CLI, the kind of setting `sc-agent.json` accepts (REQ-SAG-0019). The project/phase is the name of the project or phase a session belongs to, a block of time related to major changes, and its setting is named `project`. `SC_PROJECT` is the environment variable that sets the project/phase for every CLI (REQ-SAG-0021).

**MUST statements:**

- The environment variable for a CLI setting MUST be `<CLI>_` followed by the key path's keys in upper case joined by `__`, such as `DECIDR_WORKERS` for `workers` and `DECIDR_LOCAL_RUNNERS__LAYA__PORT` for `local_runners.laya.port`.
- Every CLI MUST take the project/phase from the `SC_PROJECT` environment variable.
- When a setting is a shared setting other than the project/phase, each CLI MUST read it from that CLI's own prefix.
- When a CLI loads configuration, a value set by an environment variable MUST pass the same validation as the same value set in a configuration file.
- A failed validation of an environment value MUST name the environment variable.

**MUST NOT statements:**

- When a variable prefixed `SC_AGENT__` is set, a CLI MUST NOT read a setting from it.
- A CLI MUST NOT read a setting from another CLI's prefix.

### Rationale

A per-CLI prefix keeps an override meant for one CLI from changing another. The project/phase has one name because one phase spans every CLI and tools outside sc-agent.

### Success Criteria

**Acceptance Criteria:**

- [ ] With `DECIDR_LOCAL_RUNNERS__LAYA__PORT=9000` set, `decidr doctor` reports the setting `local_runners.laya.port` with value 9000 and source `DECIDR_LOCAL_RUNNERS__LAYA__PORT`.
- [ ] With `DECIDR_WORKERS=3` set, `decidr doctor` reports the setting `workers` with value 3 and source `DECIDR_WORKERS`, and `sitr doctor` reports the same rows as it reports without `DECIDR_WORKERS` set.
- [ ] With `SC_PROJECT=phase-d` set, `decidr doctor` and `sitr doctor` both report the setting `project` with value `phase-d` and source `SC_PROJECT`.
- [ ] With `SC_AGENT__WORKERS=3` set, `decidr doctor` reports no setting whose source is `SC_AGENT__WORKERS`.
- [ ] With `DECIDR_WORKERS=0` set, decidr exits with an error naming `DECIDR_WORKERS`, and with `"workers": 0` in the local `decidr.json` it exits with the same check's error naming that file and `workers`.

### Dependencies

**Related:**

- REQ-SAG-0013
- REQ-SAG-0019
- ADR-SAG-0020
