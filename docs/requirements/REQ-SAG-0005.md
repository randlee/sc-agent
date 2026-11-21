# Requirement: Session log JSONL layout

**Status:** Approved  
**Version:** 0.3.2  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-20  
**Owner:** Rand Lee  

---

## REQ-SAG-0005: Session log JSONL layout

**Status:** Approved  

### Requirement Statement

Definitions: `<log_root>` is `~/.sc-agent`, or a CLI's `<CLI>_LOG_ROOT` when set; `<CLI>_LOG_ROOT` is a test override only (REQ-SAG-0003). JSON is JavaScript Object Notation (RFC 8259). JSONL is newline-delimited JSON: one JSON object per line. A CLI is a command-line interface program. The session directory is `<log_root>/sessions/<repo>/<agent_role>/<project>/`. A session's session directory is the session directory for that session's repo, agent role and project.

**MUST statements:**

- Session logs MUST be written as JSONL.
- Session logs MUST be written in the session directory.
- Every session MUST be appended to the file `sessions.jsonl` in its session directory.
- `decidr` MUST write its session logs in the session directory.

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