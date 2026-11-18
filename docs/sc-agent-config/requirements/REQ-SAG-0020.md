# Requirement: Configuration environment variable names

**Status:** Proposed  
**Version:** 0.1.0  
**Created:** 2026-11-18  
**Last Updated:** 2026-11-18  
**Owner:** Rand Lee  

---

## REQ-SAG-0020: Configuration environment variable names

**Status:** Proposed  

### Requirement Statement

Definitions: `<CLI>` is the CLI's program name in upper case with each `-` replaced by `_`, such as `DECIDR` or `SITR`. A key path is the list of key names from the top of a configuration file to a setting. `<KEY>` is a key name in upper case.

**MUST statements:**

- The environment variable for a CLI setting MUST be `<CLI>_` followed by the key path's keys in upper case joined by `__`, such as `DECIDR_WORKERS` for `workers` and `DECIDR_LOCAL_RUNNERS__LAYA__PORT` for `local_runners.laya.port`.
- The project/phase environment variable MUST be `SC_PROJECT` for every CLI (REQ-SAG-0021).
- A shared setting other than the project/phase MUST be read by each CLI from that CLI's own prefix, such as `DECIDR_LOG_ROOT` and `SITR_LOG_ROOT`.
- An environment value MUST pass the same validation as the same value in a file, and a failed check MUST name the environment variable.

**MUST NOT statements:**

- A CLI MUST NOT read settings from variables prefixed `SC_AGENT__`.
- A CLI MUST NOT read a setting from another CLI's prefix.

### Rationale

A per-CLI prefix keeps an override meant for one CLI from changing another. The project/phase has one name because one phase spans every CLI and tools outside sc-agent.

### Success Criteria

**Acceptance Criteria:**

- [ ] `DECIDR_LOCAL_RUNNERS__LAYA__PORT=9000` sets decidr's `local_runners.laya.port` to 9000.
- [ ] `DECIDR_WORKERS=3` sets decidr's workers to 3 and leaves sitr unchanged.
- [ ] `SC_AGENT__WORKERS=3` has no effect on decidr.
- [ ] `DECIDR_WORKERS=0` fails with an error naming `DECIDR_WORKERS`.

### Dependencies

**Related:**

- REQ-SAG-0013
- REQ-SAG-0019
- ADR-SAG-0020
