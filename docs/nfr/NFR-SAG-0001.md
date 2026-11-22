# Requirement: Session logging at 40,000 sessions per task

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## NFR-SAG-0001: Session logging at 40,000 sessions per task

**Status:** Approved  

### Requirement Statement

Definitions: A task is one caller run. A session is one model request with its own ULID `session_id`; every send of that request, including busy resends, belongs to the same session. One task of the owner's produced 40,000 sessions, each one Jev query.

**MUST statements:**

- All sessions of one repo, agent role and project MUST go to one file.

**MUST NOT statements:**

- Session logging MUST NOT create one log file per session.

### Rationale

The owner analyses 10,000 to 20,000 or more decisions at a time to tune consumer code, and one task has already produced 40,000 sessions.

Owner ruling (2026-11-21), Rand Lee: "My words are what I want, not excess control resulting."

### Success Criteria

**Acceptance Criteria:**

- [ ] A test task of many sessions with one repo, agent role and project writes one row per session, each with a distinct `session_id`, to one file.

### Dependencies

**Related:**

- REQ-SAG-0005
- ADR-SAG-0002

### Test Strategy

**Test Types:**

- load test