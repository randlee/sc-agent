# Requirement: Session log rows and run links

**Status:** Approved  
**Version:** 0.8.1  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0005: Session log rows and run links

**Status:** Approved  

### Requirement Statement

Definitions: JSON is JavaScript Object Notation (RFC 8259). JSONL is newline-delimited JSON: one JSON object per line. A query is one decision request sent through `send`, or one request of a `send_batch` call. A query's session directory is `<log_root>/sessions/<repo>/<agent_role>/<project>/` for the query's repo, agent role and project, with `<log_root>` as defined in REQ-SAG-0003. A query's session log is the JSONL file `sessions.jsonl` in its session directory, and its run log is the JSONL file `runs.jsonl` in the same directory. A session row is the one line of a session log that records one query sent to a model and its result. A Run is the context object through which every query is sent (ADR-SAG-0019 decision 14). A link record is the one line of a run log that ties a Run to the session row that answered one of its queries.

**MUST statements:**

- Session logs MUST be written as JSONL.
- For every query sent to a model, sc-agent MUST append a session row to the query's session log.
- For every session row that answers a Run's query, sc-agent MUST append one link record to the query's run log.


- Each new session row MUST store `ok`, `answers` and `error` instead of an opaque `result` envelope. On success `answers` contains one entry per question, with `question_id`, `kind` and the kind's typed fields; `error` is null. On failure `answers` is null and `error` retains `id`, `category`, `message`, `action` and `attempts`.
- The answer kinds MUST be `noul` (`noul`), `choice` (`choice`, `confidence`, `probabilities`), `score` (`score`, `confidence`, `legend`, `probabilities`) and `box` (`top`, `bottom`, `left`, `right`, `confidence`). Fractional values MUST remain floating point without rounding (REQ-SAG-0007); the existing API's null box/confidence values remain representable.
- Rust MUST own the per-kind answer types used for provider decoding, returned typed answers and session serialization. The wheel MUST expose these types to Python; the template and consumers MUST NOT infer an answer kind by joining the request or implement their own answer parser.
- The reader and cache MUST read both phase-j `sc-agent-session/2` rows with `result` and the new typed rows. The planned `sc-agent-session/2` tag is retained; readers distinguish its two payload shapes by their fields, and historical rows MUST NOT be rewritten. Cache identity, latest-success selection, link records and the exact event-size limit remain unchanged (REQ-SAG-0703).

**MUST NOT statements:**

- sc-agent MUST NOT append more than one session row for one query.

### Rationale

Every query from every tool lands in the session log, so a tool references a row by `session_id` instead of copying the query or the response. The run log records which Run each row answered, because one row can answer many Runs through cache hits. What a row and a link record hold, and that both logs are never rotated or pruned, is in ADR-SAG-0021 (Session log).

### Success Criteria

**Acceptance Criteria:**

- [ ] Every line of the `sessions.jsonl` that three sends write parses as one JSON object: unit test crates/sc-agent/tests/records.rs:7 `req_sag_0001_0004_0005_session_rows_append_with_distinct_ulids`.
- [ ] Three sends with `repo` `github.com/randlee/sc-agent`, `agent_role` `rust-developer` and `project` `phase-b` add three rows to one `sessions.jsonl` in that session directory: crates/sc-agent/tests/records.rs:7 `req_sag_0001_0004_0005_session_rows_append_with_distinct_ulids`.
- [ ] One Python `sc_agent.send` adds one row to a `sessions.jsonl` under `<log_root>/sessions/`: tests/contract/test_contract.py:135 `test_session_and_general_log_share_fixed_root`.
- [ ] A `decidr` CLI send with `repo` `github.com/randlee/sc-agent`, `agent_role` `rust-developer` and `project` `phase-b` adds one row to `sessions.jsonl` in that session directory: CLI run in crates/decidr/tests/cli.rs, planned in agt-phase-j.1.
- [ ] A query sent four times after three busy responses adds one row: crates/sc-agent/tests/sending.rs:186 `req_sag_0601_busy_resends_unchanged_retry_after_and_exponential_backoff`.
- [ ] A batch of 3 through a Run adds 3 link records to `runs.jsonl` in the same directory: unit test planned in agt-j-2 deliverable J2-5.

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
