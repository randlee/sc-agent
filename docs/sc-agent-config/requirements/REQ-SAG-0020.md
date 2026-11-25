# Requirement: Configuration environment variable names

**Status:** Approved  
**Version:** 0.5.0  
**Created:** 2026-11-18  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0020: Configuration environment variable names

**Status:** Approved  

### Requirement Statement

Definitions: A CLI is one of sc-agent's command-line programs that reads configuration, such as `decidr`. `<CLI>` is the CLI's program name in upper case with each `-` replaced by `_`, such as `DECIDR`. A key path is the list of key names from the top of a configuration file to a setting. `<KEY>` is a key name in upper case. A shared setting is a default that a CLI file may override, the kind of setting `sc-agent.json` accepts (REQ-SAG-0019). The project/phase is the name of the project or phase a session belongs to, a block of time related to major changes, and its setting is named `project`. `SC_PROJECT` is the environment variable that sets the project/phase for every CLI (REQ-SAG-0021).

**MUST statements:**

- The environment variable for a CLI setting MUST be `<CLI>_` followed by the key path's keys in upper case joined by `__`, such as `DECIDR_WORKERS` for `workers` and `DECIDR_LAYA__PORT` for `laya.port`.
- Every CLI MUST take the project/phase from the `SC_PROJECT` environment variable.
- When a setting is a shared setting other than the project/phase, each CLI MUST read it from that CLI's own prefix, such as `DECIDR_WORKERS`.
- When a CLI loads configuration, a value set by an environment variable MUST pass the same validation as the same value set in a configuration file.
- A failed validation of an environment value MUST name the environment variable.
- An environment value MUST be parsed as JSON when valid JSON, otherwise as a string. For example, `DECIDR_MODELS__LAYA__ID=1` is a number, like `"id": 1` in a file, and fails the string id validation; a quoted JSON string is accepted.

**MUST NOT statements:**

- A CLI MUST NOT read a setting from another CLI's prefix.

### Rationale

A per-CLI prefix keeps an override meant for one CLI from changing another. The project/phase has one name because one phase spans every CLI and tools outside sc-agent.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

Owner ruling 2026-11-24 (plan review, ADR-SAG-0003): a local endpoint's model id moved from `laya.model` to the model entry `models.laya.id` (REQ-SAG-0019), so the example names the new key path.

Owner ruling 2026-11-24 (supersedes the optional or configured local model id and the omitted `model` above): laya and decider have fixed model ids in the model table, like every other model. laya requires `laya:en`. decider has two rows, `decider-2b-v11` (port 18000) and `decider-4b-v2.1` (port 18001), both released 2026-09-24, and `decider` resolves by the same rule as `haiku`. The decider servers accept and ignore `model`, so every request carries the resolved id. Cache keys always use the resolved fixed id.

### Success Criteria

**Acceptance Criteria:**

- [ ] With `DECIDR_LAYA__PORT=9000` set, `decidr doctor` reports the setting `laya.port` with value 9000 and source `DECIDR_LAYA__PORT`.
- [ ] With `DECIDR_WORKERS=3` set, `decidr doctor` reports the setting `workers` with value 3 and source `DECIDR_WORKERS`.
- [ ] With `SC_PROJECT=phase-d` set, `decidr doctor` reports the setting `project` with value `phase-d` and source `SC_PROJECT`.
- [ ] With `DECIDR_LOG_ROOT=/tmp/x` set, `decidr doctor` reports the fixed log root as the value `log_root: "/tmp/x"`, separately from configuration settings (ADR-SAG-0021).
- [ ] With `DECIDR_WORKERS=0` set, decidr exits with an error naming `DECIDR_WORKERS`, and with `"workers": 0` in the local `decidr.json` it exits with the same check's error naming that file and `workers`.

### Dependencies

**Related:**

- REQ-SAG-0013
- REQ-SAG-0019
- ADR-SAG-0020
