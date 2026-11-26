# Requirement: Log locations and retention

**Status:** Approved  
**Version:** 0.9.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0003: Log locations and retention

**Status:** Approved  

### Requirement Statement

Definitions: sc-agent is the crates and CLIs of this repository. A CLI is one of the command-line programs decidr or sitr; `<cli>` is its name in lower case and `<CLI>` in upper case. A Logger is an sc-observability logger that a caller passes to a CLI's library entry point. The general log is the diagnostic log of one entry point (ADR-SAG-0021); decidr's is `decidr.log.jsonl` and sitr's is `sitr.log.jsonl`. A session log is the record of every session; a session is one model request with its own ULID `session_id`, and every send of that request, including busy resends, belongs to it. A link record ties one run to one session (REQ-SAG-0005). A run log is the file of link records that sits beside a session log in the same directory. To remove a log is to rotate, prune or delete it. The generated client is the Python module a consumer renders from the sc-decidr wheel's sc-compose template (REQ-SAG-0018). The client name is the consumer's product name, a constant in the generated client, and equals its `agent_role` (REQ-SAG-0004), such as `plan-anchor` or `judge-jev`. The client Logger is the Logger the generated client obtains from the wheel under the client name (REQ-SAG-0008). `<CLIENT>_LOG_ROOT` is the environment variable named by the client name uppercased with hyphens replaced by underscores. The default session files are the session files REQ-SAG-0005 names for an agent role. The client's logs are the general log the client Logger writes and the default session files.

**MUST statements:**

- With no Logger injected, each CLI MUST write its general log to `~/.sc-agent/logs/<cli>.log.jsonl`.
- When `<CLI>_LOG_ROOT` is set and non-empty, that CLI MUST use its value in place of `~/.sc-agent`.
- When `<CLIENT>_LOG_ROOT` is set and not empty, the generated client MUST use that folder in place of `~/.sc-agent/` for its logs.
- Session logs MUST be written under `~/.sc-agent/sessions/` (REQ-SAG-0005).
- A session log MUST remain in `~/.sc-agent/sessions/` whatever its age.
- A run log MUST remain in `~/.sc-agent/sessions/` whatever its age.

**MUST NOT statements:**

- sc-agent MUST NOT contain code that removes a general log.
- sc-agent MUST NOT remove a session log.
- sc-agent MUST NOT remove a run log.

### Rationale

Session logs are the long-term record used to analyse tens of thousands of decisions, so losing any of them loses data. Link records are append-only and immutable, so the run log is kept the same way.

### Success Criteria

**Acceptance Criteria:**

- [ ] A CLI run of decidr with nothing configured writes `~/.sc-agent/logs/decidr.log.jsonl`.
- [ ] A CLI run of sitr with nothing configured writes `~/.sc-agent/logs/sitr.log.jsonl`.
- [ ] A CLI run of decidr with `DECIDR_LOG_ROOT` set to `<dir>` writes `<dir>/logs/decidr.log.jsonl`.
- [ ] A CLI run of decidr with `DECIDR_LOG_ROOT` set to `<dir>` writes its session log under `<dir>/sessions/`.
- [ ] A unit test sets `<CLIENT>_LOG_ROOT` to a test folder, makes one sc-decidr call, and finds the general log and session rows under that folder and nothing under `~/.sc-agent/`.
- [ ] A CLI run of decidr with nothing configured writes its session log under `~/.sc-agent/sessions/`.
- [ ] A Rust unit test shows a session log older than the sc-observability retention age is still present after a new session.
- [ ] A Rust unit test shows a run log older than the sc-observability retention age is still present after a new session.
- [ ] Inspection of `rg -n 'rotat|prune|retention' crates/` shows no code that removes a general log.
- [ ] A Rust unit test shows a session log is unchanged and present after decidr runs a new session.
- [ ] A Rust unit test shows a run log's earlier lines are unchanged after decidr runs a new session.
- [ ] A Rust unit test shows a session log past the sc-observability rotation size has no rotated copy after a new session.
- [ ] A Rust unit test shows a run log past the sc-observability rotation size has no rotated copy after a new session.

### Dependencies

**Related:**

- ADR-SAG-0002
- ADR-SAG-0021
- REQ-SAG-0005
