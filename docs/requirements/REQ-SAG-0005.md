# Requirement: Session log rows and run links

**Status:** Approved  
**Version:** 0.8.2  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-25  
**Owner:** Rand Lee  

---

## REQ-SAG-0005: Session log rows and run links

**Status:** Approved  

### Requirement Statement

Definitions: JSON is JavaScript Object Notation (RFC 8259). JSONL is newline-delimited JSON: one JSON object per line. A query is one decision request sent through `send`, or one request of a `send_batch` call. A query's session log is the JSONL file `sessions.jsonl`, and its run log is the JSONL file `runs.jsonl` in the same directory. A session row is the one line of a session log that records one query whose model call succeeded and its result. A failed call is one that ends in an error (auth, busy, quota, network or cancel). A Run is the context object through which every query is sent (ADR-SAG-0019 decision 14). A link record is the one line of a run log that ties a Run to the session row that answered one of its queries.

**MUST statements:**

- Session logs MUST be written as JSONL.
- For every query whose model call succeeded, sc-agent MUST append a session row to the query's session log.

**MUST NOT statements:**

- sc-agent MUST NOT append more than one session row for one query.
- sc-agent MUST NOT append a session row for a failed call.

### Rationale

Every successfully answered query from every tool lands in the session log, while errors go to the general log, so a tool references a row by `session_id` instead of copying the query or the response. The run log records which Run each row answered, because one row can answer many Runs through cache hits. What a row and a link record hold, and that both logs are never rotated or pruned, is in ADR-SAG-0021 (Session log).

### Success Criteria

**Acceptance Criteria:**

- [ ] Every line of the `sessions.jsonl` that three sends write parses as one JSON object.
- [ ] Three sends with `repo` `github.com/randlee/sc-agent`, `agent_role` `rust-developer` and `project` `phase-b` add three rows to one `sessions.jsonl`.
- [ ] One Python `sc_agent.send` adds one row to a `sessions.jsonl` under `<log_root>/sessions/`.
- [ ] A `decidr` CLI send with `repo` `github.com/randlee/sc-agent`, `agent_role` `rust-developer` and `project` `phase-b` adds one row to `sessions.jsonl`.
- [ ] A query sent four times after three busy responses adds one row.
- [ ] A send that fails with a 401 adds no row to `sessions.jsonl`.
- [ ] A batch of 3 through a Run adds 3 link records to `runs.jsonl` in the same directory.

### Dependencies

**Related:**

- REQ-SAG-0003
- REQ-SAG-0004
- REQ-SAG-0102
- REQ-SAG-0601
- ADR-SAG-0002
- ADR-SAG-0019
- ADR-SAG-0021
