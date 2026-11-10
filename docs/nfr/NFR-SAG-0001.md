# Requirement: Session logging at 40,000 sessions per task

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## NFR-SAG-0001: Session logging at 40,000 sessions per task

**Status:** Draft  

### Requirement Statement

Definitions: A task is one caller run. A session is one model request with its own ULID `session_id`; every send of that request, including busy resends, belongs to the same session. One task of the owner's produced 40,000 sessions, each one Jev query.

**MUST statements:**

- Session logging MUST record one row for every session of a task of 40,000 sessions.

**MUST NOT statements:**

- A task of 40,000 sessions MUST NOT create one log file per session.

### Rationale

The owner analyses 10,000 to 20,000 or more decisions at a time to tune consumer code, and one task has already produced 40,000 sessions.

### Success Criteria

**Acceptance Criteria:**

- [ ] A test task of 40,000 sessions writes 40,000 rows, each with a distinct `session_id`.
- [ ] The same test task writes fewer session log files than it has sessions.

### Dependencies

**Related:**

- REQ-SAG-0005
- ADR-SAG-0002

### Test Strategy

**Test Types:**

- load test