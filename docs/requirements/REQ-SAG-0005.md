# Requirement: Session log JSONL layout

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0005: Session log JSONL layout

**Status:** Draft  

### Requirement Statement

Definitions: `<log_root>` is the session log root folder. JSONL is newline-delimited JSON, one row per line.

**MUST statements:**

- Session logs MUST be written as JSONL under `<log_root>/<repo>/<agent_role>/<project>/`.
- A session log row MUST be flushed when it is written.

### Rationale

Directory separation lets tools select a repository, role or project with a path glob without opening files. Flushing each row lets tools follow a running task.

### Success Criteria

**Acceptance Criteria:**

- [ ] After a session with `repo` `github.com/randlee/sc-agent`, `agent_role` `rust-developer` and `project` `phase-b`, its rows are in a file under `<log_root>/github.com/randlee/sc-agent/rust-developer/phase-b/`.
- [ ] While a session runs, each completed send's row is readable in the file before the next send starts.

### Dependencies

**Related:**

- REQ-SAG-0004
- ADR-SAG-0002