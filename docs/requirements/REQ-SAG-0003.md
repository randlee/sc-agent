# Requirement: Log retention

**Status:** Approved  
**Version:** 0.3.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-18  
**Owner:** Rand Lee  

---

## REQ-SAG-0003: Log retention

**Status:** Approved  

### Requirement Statement

Definitions: The general log is the application diagnostic log. A session log is the record of every session. A session is one model request with its own ULID `session_id`; every send of that request, including busy resends, belongs to the same session. sc-observability is the logging library sc-agent uses. The retained-log policy is the rotation and retention built into sc-observability. <log_root> is the directory under which sc-agent writes its general logs and session logs. A CLI is a command-line program of sc-agent, such as decidr or sitr. `~/.sc-agent` is the user's home directory joined with `.sc-agent` on every platform. A layer is one of the seven configuration sources of REQ-SAG-0019, looked up in order from the CLI flag to the built-in default; the first layer that sets a setting gives its value. A general logger is injected when the caller passes its own sc-observability `Logger` into the sc-agent call (REQ-SAG-0008). `<cli>` is a CLI's program name. `<client>` is the generated client's name, such as plan-anchor or judge-jev. The generated client is the Python module a consumer renders from the sc-decidr wheel's sc-compose template (REQ-SAG-0018).

**MUST statements:**

- The general log MUST use the sc-observability retained-log policy.
- A session log MUST remain in `<log_root>/sessions/` whatever its age.
- The built-in default of the `log_root` setting, layer 7 of REQ-SAG-0019, MUST be `~/.sc-agent`.
- When a layer sets `log_root`, <log_root> MUST be the value of the first layer that sets it.
- When no general logger is injected, each CLI MUST write its general log to `<log_root>/logs/<cli>/<cli>.log.jsonl`.
- When no general logger is injected, decidr MUST write its general log to `<log_root>/logs/decidr/decidr.log.jsonl`.
- When no general logger is injected, sitr MUST write its general log to `<log_root>/logs/sitr/sitr.log.jsonl`.
- A generated client MUST write its general log to `<log_root>/logs/<client>/<client>.log.jsonl`, such as `<log_root>/logs/plan-anchor/plan-anchor.log.jsonl` (REQ-SAG-0008).
- Session logs MUST be written under `<log_root>/sessions/` (REQ-SAG-0005).
- Each CLI MUST write its own general log file.
- decidr MUST write its own general log file.
- sitr MUST write its own general log file.

**MUST NOT statements:**

- sc-agent MUST NOT implement its own rotation of the general log.
- sc-agent MUST NOT implement its own retention of the general log.
- sc-agent MUST NOT rotate a session log.
- sc-agent MUST NOT prune session logs.
- sc-agent MUST NOT delete a session log.
- A general log MUST NOT be written under `<log_root>/sessions/`.
- A session log MUST NOT be written under `<log_root>/logs/`.

### Rationale

sc-observability already rotates and prunes the general log. Session logs are the long-term record used to analyse tens of thousands of decisions, so losing any of them loses data.

### Success Criteria

**Acceptance Criteria:**

- [ ] The general log rotates under the sc-observability retained-log policy, and no sc-agent crate contains rotation or pruning code.
- [ ] A session log file under `<log_root>/sessions/` whose modification time is older than the sc-observability retention age is still present and unchanged after decidr runs a new session.
- [ ] With no layer setting `log_root`, `decidr doctor` and `sitr doctor` each report `log_root` as `<home>/.sc-agent` with source `default`.
- [ ] With `DECIDR_LOG_ROOT` set to a directory `<dir>`, a decidr run writes its general log to `<dir>/logs/decidr/decidr.log.jsonl` and its session log under `<dir>/sessions/`.
- [ ] With `log_root` set in the global `sc-agent.json` and `DECIDR_LOG_ROOT` set to `<dir>`, `decidr doctor` reports `log_root` as `<dir>` with source `DECIDR_LOG_ROOT`.
- [ ] With no general logger injected, a sitr run writes its general log to `<log_root>/logs/sitr/sitr.log.jsonl`.
- [ ] After the plan-anchor generated client runs, its general log is `<log_root>/logs/plan-anchor/plan-anchor.log.jsonl`.
- [ ] With no log_root configured, decidr writes its general log to `<home>/.sc-agent/logs/decidr/decidr.log.jsonl` and its session logs under `<home>/.sc-agent/sessions/`.
- [ ] After decidr and sitr each run, `<log_root>/logs/decidr/` and `<log_root>/logs/sitr/` each hold only that CLI's general log, and `<log_root>/logs/` holds no session log.

### Dependencies

**Related:**

- ADR-SAG-0002
- ADR-SAG-0021
- REQ-SAG-0008
- REQ-SAG-0019