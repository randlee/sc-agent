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

Definitions: A CLI is one of sc-agent's command-line programs that reads configuration, such as `decidr`.

**MUST statements:**

- When a CLI loads configuration, a value set by an environment variable MUST pass the same validation as the same value set in a configuration file.
- When an environment value fails validation, the error MUST include the name of the environment variable that set the value.
- When an environment value is valid JSON, it MUST be parsed as JSON.
- When an environment value is not valid JSON, it MUST be taken as a string.

**MUST NOT statements:**

- A CLI MUST NOT read a setting from another CLI's prefix.

### Rationale

A per-CLI prefix keeps an override meant for one CLI from changing another.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] With `DECIDR_LOG_ROOT=/tmp/x` set, `decidr doctor` reports the fixed log root as the value `log_root: "/tmp/x"`, separately from configuration settings (ADR-SAG-0021).
- [ ] With `DECIDR_WORKERS=0` set, decidr exits with an error naming `DECIDR_WORKERS`, and with `"workers": 0` in the local `decidr.json` it exits with the same check's error naming that file and `workers`.
- [ ] A unit test sets `DECIDR_LAYA__HOST=1` and checks the load fails the string host validation, and sets `DECIDR_LAYA__HOST="h"` and checks the host is `h`.
- [ ] A unit test sets `workers` under another CLI's prefix and checks decidr's loaded `workers` keeps its default.

### Dependencies

**Related:**

- REQ-SAG-0013
- REQ-SAG-0019
- ADR-SAG-0020
