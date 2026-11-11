# Requirement: Session log JSONL layout

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-10  
**Owner:** Rand Lee  
**Revision:** 0.2.0 (2026-11-10, owner): session logs move under <log_root>/logs/.  

---

## REQ-SAG-0005: Session log JSONL layout

**Status:** Approved  

### Requirement Statement

Definitions: `<log_root>` is the session log root folder. JSONL is newline-delimited JSON, one row per line.

**MUST statements:**

- Session logs MUST be written as JSONL under `<log_root>/logs/<repo>/<agent_role>/<project>/`.
- All sessions of one repo, agent role and project MUST be appended to one JSONL file in that directory.

### Rationale

Directory separation lets tools select a repository, role or project with a path glob without opening files.

### Success Criteria

**Acceptance Criteria:**

- [ ] After a session with `repo` `github.com/randlee/sc-agent`, `agent_role` `rust-developer` and `project` `phase-b`, its rows are in a file under `<log_root>/logs/github.com/randlee/sc-agent/rust-developer/phase-b/`.

### Dependencies

**Related:**

- REQ-SAG-0004
- ADR-SAG-0002