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

Definitions: JSON is JavaScript Object Notation (RFC 8259). JSONL is newline-delimited JSON: one JSON object per line. A query is one decision request sent through `send`, or one request of a `send_batch` call. A query's session log is the JSONL file `sessions.jsonl`, and its run log is the JSONL file `runs.jsonl` in the same directory. A session row is the one line of a session log that records one query sent to a model and its result. A Run is the context object through which every query is sent (ADR-SAG-0019 decision 14). A link record is the one line of a run log that ties a Run to the session row that answered one of its queries.

**MUST statements:**

- Session logs MUST be written as JSONL.
- For every query sent to a model, sc-agent MUST append a session row to the query's session log.
- For every session row that answers a Run's query, sc-agent MUST append one link record to the query's run log.

**MUST NOT statements:**

- sc-agent MUST NOT append more than one session row for one query.

### Rationale

Every query from every tool lands in the session log, so a tool references a row by `session_id` instead of copying the query or the response. The run log records which Run each row answered, because one row can answer many Runs through cache hits. What a row and a link record hold, and that both logs are never rotated or pruned, is in ADR-SAG-0021 (Session log).

### Success Criteria

**Acceptance Criteria:**

- [ ] Every line of the `sessions.jsonl` that three sends write parses as one JSON object: unit test crates/sc-agent/tests/records.rs:7 `req_sag_0001_0004_0005_session_rows_append_with_distinct_ulids`.
- [ ] Three sends with `repo` `github.com/randlee/sc-agent`, `agent_role` `rust-developer` and `project` `phase-b` add three rows to one `sessions.jsonl`: crates/sc-agent/tests/records.rs:7 `req_sag_0001_0004_0005_session_rows_append_with_distinct_ulids`.
- [ ] One Python `sc_agent.send` adds one row to a `sessions.jsonl` under `<log_root>/sessions/`: tests/contract/test_contract.py:135 `test_session_and_general_log_share_fixed_root`.
- [ ] A `decidr` CLI send with `repo` `github.com/randlee/sc-agent`, `agent_role` `rust-developer` and `project` `phase-b` adds one row to `sessions.jsonl`: CLI run in crates/decidr/tests/cli.rs, planned in agt-phase-j.1.
- [ ] A query sent four times after three busy responses adds one row: crates/sc-agent/tests/sending.rs:186 `req_sag_0601_busy_resends_unchanged_retry_after_and_exponential_backoff`.
- [ ] A batch of 3 through a Run adds 3 link records to `runs.jsonl` in the same directory: unit test planned in agt-j-3 deliverable J3-5.

### Dependencies

**Related:**

- REQ-SAG-0003
- REQ-SAG-0004
- REQ-SAG-0102
- REQ-SAG-0601
- REQ-SAG-0703
- ADR-SAG-0002
- ADR-SAG-0019
- ADR-SAG-0021
