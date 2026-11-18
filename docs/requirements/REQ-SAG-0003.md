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

Definitions: The general log is the application diagnostic log. A session log is the record of every session. A session is one model request with its own ULID `session_id`; every send of that request, including busy resends, belongs to the same session. sc-observability is the logging library sc-agent uses.

**MUST statements:**

- The general log MUST use the rotation and retention built into sc-observability.
- Session logs MUST be kept indefinitely.
- <log_root> MUST default to `~/.sc-agent`, the user's home directory joined with `.sc-agent` on every platform, and MUST be settable as `log_root` through the layers of REQ-SAG-0019.
- A CLI's general log MUST be written at `<log_root>/logs/<cli>/<cli>.log.jsonl`, such as `<log_root>/logs/decidr/decidr.log.jsonl`.
- Session logs MUST be written under `<log_root>/sessions/` (REQ-SAG-0005).
- Each CLI MUST write its own general log file.

**MUST NOT statements:**

- sc-agent MUST NOT implement its own rotation or retention for the general log.
- sc-agent MUST NOT rotate, prune or delete session logs.
- A general log MUST NOT be written under `<log_root>/sessions/`, and a session log MUST NOT be written under `<log_root>/logs/`.

### Rationale

sc-observability already rotates and prunes the general log. Session logs are the long-term record used to analyse tens of thousands of decisions, so losing any of them loses data.

### Success Criteria

**Acceptance Criteria:**

- [ ] The general log rotates under the sc-observability retained-log policy, and no sc-agent crate contains rotation or pruning code.
- [ ] A session log written more than the sc-observability retention age ago is still present after a new session runs.
- [ ] With no log_root configured, decidr writes its general log to `<home>/.sc-agent/logs/decidr/decidr.log.jsonl` and its session logs under `<home>/.sc-agent/sessions/`.
- [ ] After decidr and sitr each run, `<log_root>/logs/decidr/` and `<log_root>/logs/sitr/` each hold only that CLI's general log, and `<log_root>/logs/` holds no session log.

### Dependencies

**Related:**

- ADR-SAG-0002
- ADR-SAG-0021
- REQ-SAG-0019