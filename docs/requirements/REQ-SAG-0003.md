# Requirement: Log retention

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-10  
**Owner:** Rand Lee  
**Revision:** 0.2.0 (2026-11-10, owner): <log_root> defaults to ~/.sc-agent on every platform, and every log is written under <log_root>/logs/.  

---

## REQ-SAG-0003: Log retention

**Status:** Approved  

### Requirement Statement

Definitions: The general log is the application diagnostic log. A session log is the record of every session. A session is one model request with its own ULID `session_id`; every send of that request, including busy resends, belongs to the same session. sc-observability is the logging library sc-agent uses.

**MUST statements:**

- The general log MUST use the rotation and retention built into sc-observability.
- Session logs MUST be kept indefinitely.
- <log_root> MUST default to `~/.sc-agent`, the user's home directory joined with `.sc-agent` on every platform.
- Every log MUST be written under `<log_root>/logs/`: the general log at `<log_root>/logs/<cli>.log.jsonl` and session logs at `<log_root>/logs/<repo>/<agent_role>/<project>/`.
- Each CLI MUST write its own general log file.

**MUST NOT statements:**

- sc-agent MUST NOT implement its own rotation or retention for the general log.
- sc-agent MUST NOT rotate, prune or delete session logs.

### Rationale

sc-observability already rotates and prunes the general log. Session logs are the long-term record used to analyse tens of thousands of decisions, so losing any of them loses data.

### Success Criteria

**Acceptance Criteria:**

- [ ] The general log rotates under the sc-observability retained-log policy, and no sc-agent crate contains rotation or pruning code.
- [ ] A session log written more than the sc-observability retention age ago is still present after a new session runs.
- [ ] With no log_root configured, decide writes its general log and session logs under `<home>/.sc-agent/logs/`.

### Dependencies

**Related:**

- ADR-SAG-0002