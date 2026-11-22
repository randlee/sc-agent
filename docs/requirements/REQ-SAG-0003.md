# Requirement: Log locations and retention

**Status:** Approved  
**Version:** 0.6.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-22  
**Owner:** Rand Lee  

---

## REQ-SAG-0003: Log locations and retention

**Status:** Approved  

### Requirement Statement

Definitions: The general log is the diagnostic log of one entry point (ADR-SAG-0021). A session log is the record of every session; a session is one model request with its own ULID `session_id`, and every send of that request, including busy resends, belongs to it. The retained-log policy is the rotation and retention built into sc-observability.

**MUST statements:**

- With no Logger injected, each CLI MUST write its general log to `~/.sc-agent/logs/<cli>.log.jsonl`: `decidr.log.jsonl` for decidr, `sitr.log.jsonl` for sitr.
- When `<CLI>_LOG_ROOT` is set and non-empty, that CLI MUST use its value in place of `~/.sc-agent`.
- Session logs MUST be written under `~/.sc-agent/sessions/` (REQ-SAG-0005).
- The general log MUST use the sc-observability retained-log policy.
- A session log MUST remain in `~/.sc-agent/sessions/` whatever its age.

**MUST NOT statements:**

- sc-agent MUST NOT implement its own rotation or retention of the general log.
- sc-agent MUST NOT rotate, prune or delete a session log.
- A general log MUST NOT be written under `~/.sc-agent/sessions/`, and a session log MUST NOT be written under `~/.sc-agent/logs/`.

### Rationale

sc-observability already rotates and prunes the general log. Session logs are the long-term record used to analyse tens of thousands of decisions, so losing any of them loses data.

Owner ruling (2026-11-21), Rand Lee: "My words are what I want, not excess control resulting."

### Success Criteria

**Acceptance Criteria:**

- [ ] With nothing configured, decidr writes `~/.sc-agent/logs/decidr.log.jsonl` and sitr writes `~/.sc-agent/logs/sitr.log.jsonl`, and `~/.sc-agent/logs/` holds no session log.
- [ ] With `DECIDR_LOG_ROOT` set to `<dir>`, decidr writes `<dir>/logs/decidr.log.jsonl` and its session log under `<dir>/sessions/`.
- [ ] The general log rotates under the sc-observability retained-log policy, and no sc-agent crate contains rotation or pruning code.
- [ ] A session log file whose modification time is older than the sc-observability retention age is still present and unchanged after decidr runs a new session.

### Dependencies

**Related:**

- ADR-SAG-0002
- ADR-SAG-0021
- REQ-SAG-0005
