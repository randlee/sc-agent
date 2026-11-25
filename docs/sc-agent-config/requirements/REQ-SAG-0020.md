# Requirement: Configuration environment variable names

**Status:** Approved  
**Version:** 0.7.0  
**Created:** 2026-11-18  
**Last Updated:** 2026-11-25  
**Owner:** Rand Lee  

---

## REQ-SAG-0020: Configuration environment variable names

**Status:** Approved  

### Requirement Statement

Definitions: A CLI is one of sc-agent's command-line programs that reads configuration, such as `decidr`. `<CLI>` is the CLI's program name in upper case with each `-` replaced by `_`, such as `DECIDR`. A key path is the list of key names from the top of a configuration file to a setting. `<KEY>` is a key name in upper case. A shared setting is a default that a CLI file may override, the kind of setting `sc-agent.json` accepts (REQ-SAG-0019). The project/phase is the name of the project or phase a session belongs to, a block of time related to major changes, and its setting is named `project`. `SC_PROJECT` is the environment variable that sets the project/phase for every CLI (REQ-SAG-0021).

**MUST statements:**

- The environment variable name for a CLI setting MUST be `<CLI>_` followed by the key path joined by `__` in upper case.
- Every CLI MUST take the project/phase from the `SC_PROJECT` environment variable.
- When a setting is a shared setting other than the project/phase, each CLI MUST read it from that CLI's own prefix, such as `DECIDR_WORKERS`.
- When a CLI loads configuration, a value set by an environment variable MUST pass the same validation as the same value set in a configuration file.
- A failed validation of an environment value MUST name the environment variable.
- An environment value MUST be parsed as JSON when valid JSON, otherwise as a string.

**MUST NOT statements:**

- A CLI MUST NOT read a setting from another CLI's prefix.

### Rationale

A per-CLI prefix keeps an override meant for one CLI from changing another. The project/phase has one name because one phase spans every CLI and tools outside sc-agent.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] With `DECIDR_LAYA__PORT=9000` set, `decidr doctor` reports the setting `laya.port` with value 9000 and source `DECIDR_LAYA__PORT`.
- [ ] With `DECIDR_WORKERS=3` set, `decidr doctor` reports the setting `workers` with value 3 and source `DECIDR_WORKERS`.
- [ ] With `SC_PROJECT=phase-d` set, `decidr doctor` reports the setting `project` with value `phase-d` and source `SC_PROJECT`.
- [ ] With `DECIDR_LOG_ROOT=/tmp/x` set, `decidr doctor` reports the fixed log root as the value `log_root: "/tmp/x"`, separately from configuration settings (ADR-SAG-0021).
- [ ] With `DECIDR_WORKERS=0` set, decidr exits with an error naming `DECIDR_WORKERS`, and with `"workers": 0` in the local `decidr.json` it exits with the same check's error naming that file and `workers`.
- [ ] A unit test checks that the environment variable name for `workers` in decidr is `DECIDR_WORKERS` and for `laya.port` is `DECIDR_LAYA__PORT`.
- [ ] A unit test sets `DECIDR_LAYA__HOST=1` and checks the load fails the string host validation, and sets `DECIDR_LAYA__HOST="h"` and checks the host is `h`.
- [ ] A unit test sets `workers` under another CLI's prefix and checks decidr's loaded `workers` keeps its default.

### Dependencies

**Related:**

- REQ-SAG-0013
- REQ-SAG-0019
- ADR-SAG-0020
