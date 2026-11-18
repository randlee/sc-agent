# Requirement: Session log JSONL layout

**Status:** Approved  
**Version:** 0.3.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-18  
**Owner:** Rand Lee  

---

## REQ-SAG-0005: Session log JSONL layout

**Status:** Approved  

### Requirement Statement

Definitions: `<log_root>` is the session log root folder. JSONL is newline-delimited JSON, one row per line.

**MUST statements:**

- Session logs MUST be written as JSONL under `<log_root>/sessions/<repo>/<agent_role>/<project>/`.
- All sessions of one repo, agent role and project MUST be appended to the one file `sessions.jsonl` in that directory.
- Every CLI MUST write its session logs into this one tree.

### Rationale

Directory separation lets tools select a repository, role or project with a path glob without opening files.

### Success Criteria

**Acceptance Criteria:**

- [ ] After a session with `repo` `github.com/randlee/sc-agent`, `agent_role` `rust-developer` and `project` `phase-b`, its rows are in a file in `<log_root>/sessions/github.com/randlee/sc-agent/rust-developer/phase-b/sessions.jsonl`.

### Dependencies

**Related:**

- REQ-SAG-0004
- ADR-SAG-0002
- ADR-SAG-0021
- REQ-SAG-0003