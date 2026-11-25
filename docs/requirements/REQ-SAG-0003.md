# Requirement: Log locations and retention

**Status:** Approved  
**Version:** 0.8.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0003: Log locations and retention

**Status:** Approved  

### Requirement Statement

Definitions: The general log is the diagnostic log of one entry point (ADR-SAG-0021). A session log is the record of every session; a session is one model request with its own ULID `session_id`, and every send of that request, including busy resends, belongs to it. The retained-log policy is the rotation and retention built into sc-observability. The session sink is the sc-observability sink through which sc-agent writes session rows and link records. A run log is the `runs.jsonl` file of link records that sits beside a `sessions.jsonl` in the same directory (REQ-SAG-0005).

**MUST statements:**

- With no Logger injected, each CLI MUST write its general log to `~/.sc-agent/logs/<cli>.log.jsonl`: `decidr.log.jsonl` for decidr, `sitr.log.jsonl` for sitr.
- When `<CLI>_LOG_ROOT` is set and non-empty, that CLI MUST use its value in place of `~/.sc-agent`.
- Session logs MUST be written under `~/.sc-agent/sessions/` (REQ-SAG-0005).
- Session rows MUST be written in Rust through sc-observability, by the session sink.
- The session sink MUST append to the session log file.
- Link records MUST be written in Rust through sc-observability, by the session sink.
- The session sink MUST append to the run log file.
- A run log MUST remain in `~/.sc-agent/sessions/` whatever its age.
- The general log MUST use the sc-observability retained-log policy.
- A session log MUST remain in `~/.sc-agent/sessions/` whatever its age.

**MUST NOT statements:**

- sc-agent MUST NOT implement its own rotation or retention of the general log.
- sc-agent MUST NOT rotate, prune or delete a session log.
- The retained-log policy MUST NOT apply to a session log.
- The session sink MUST NOT rotate or prune a session log.
- sc-agent MUST NOT rotate, prune or delete a run log.
- The retained-log policy MUST NOT apply to a run log.
- sc-agent MUST NOT offer any setting, argument or environment variable that turns the run log off.
- A general log MUST NOT be written under `~/.sc-agent/sessions/`, and a session log MUST NOT be written under `~/.sc-agent/logs/`.

### Rationale

sc-observability already rotates and prunes the general log. Session logs are the long-term record used to analyse tens of thousands of decisions, so losing any of them loses data.

Owner ruling (2026-11-24, round 3), Rand Lee, recorded in `docs/plans/phase-j/audit-trail-log-normalization-SQLite.md`: `runs.jsonl` sits beside `sessions.jsonl`, in the same directory, is written the same way and is never rotated.

### Success Criteria

**Acceptance Criteria:**

- [ ] With nothing configured, decidr writes `~/.sc-agent/logs/decidr.log.jsonl` and sitr writes `~/.sc-agent/logs/sitr.log.jsonl`, and `~/.sc-agent/logs/` holds no session log.
- [ ] With `DECIDR_LOG_ROOT` set to `<dir>`, decidr writes `<dir>/logs/decidr.log.jsonl` and its session log under `<dir>/sessions/`.
- [ ] The general log rotates under the sc-observability retained-log policy, and no sc-agent crate contains rotation or pruning code.
- [ ] A Rust test shows session rows reach the session log through an sc-observability Logger and its session sink.
- [ ] After a session log grows past the sc-observability rotation size, decidr's next session appends to the same `sessions.jsonl`, and no rotated copy of it exists.
- [ ] A session log file whose modification time is older than the sc-observability retention age is still present and unchanged after decidr runs a new session.
- [ ] After decidr runs a session, `runs.jsonl` is in the same directory as that session's `sessions.jsonl`, and a Rust test shows its link records reach it through the session sink.
- [ ] A run log file whose modification time is older than the sc-observability retention age is still present after decidr runs a new session, with its earlier lines unchanged.

### Dependencies

**Related:**

- ADR-SAG-0002
- ADR-SAG-0021
- REQ-SAG-0005
