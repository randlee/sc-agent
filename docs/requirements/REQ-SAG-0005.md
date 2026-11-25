# Requirement: Session log rows and run links

**Status:** Approved  
**Version:** 0.8.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0005: Session log rows and run links

**Status:** Approved  

### Requirement Statement

Definitions: A query is one decision request sent through `send`, or one request of a `send_batch` call. A query's session directory is `<log_root>/sessions/<repo>/<agent_role>/<project>/` for the query's repo, agent role and project, with `<log_root>` as defined in REQ-SAG-0003. A query's session log is the file `sessions.jsonl` in its session directory, and its run log is the file `runs.jsonl` in the same directory. A session row is the one line of a session log that records one query sent to a model and its result. A row's `model_requested` is the model id of the query's resolved model (REQ-SAG-0102), and its `model_answered` is the model id that the response reported, or null when no response was received. A Run is the context object through which every query is sent (ADR-SAG-0019 decision 14). A link record is the one line `{run_id, session_id, cached}` of a run log that ties a Run to the session row that answered one of its queries; `cached` is true when that row answered through a cache hit (REQ-SAG-0703).

**MUST statements:**

- For every query sent to a model, sc-agent MUST append a session row to the query's session log.
- For every session row that answers a Run's query, sc-agent MUST append one link record to the query's run log.

**MUST NOT statements:**

- sc-agent MUST NOT append more than one session row for one query.

### Rationale

Every query from every tool lands in the session log, so a tool references a row by `session_id` instead of copying the query or the response, and the run log records which Run each row answered because one row can answer many Runs through cache hits. The session log records what happened: a query gets its row whether or not a response arrived and whichever model answered. When the answering model differs from the requested one, the call returns `decision.model_mismatch` and its row is an error row, which is never a cache hit (REQ-SAG-0703). Busy resends of a query belong to its one row (REQ-SAG-0003, REQ-SAG-0601).

The rest of what a row holds is listed in agt-j-2 deliverable J2-2. The 1 MiB limit is in ADR-SAG-0021 (Errors): a session event of at most 1 MiB is written whole, and a larger event or a failed write writes no row and returns `log.session_write`. That the session log and the run log are always on and never rotated or pruned is in ADR-SAG-0021 (Session log) and REQ-SAG-0003.

### Success Criteria

**Acceptance Criteria:**

- [ ] Three sends with `repo` `github.com/randlee/sc-agent`, `agent_role` `rust-developer` and `project` `phase-b` add three rows to one `sessions.jsonl` in that session directory: crates/sc-agent/tests/records.rs:7 `req_sag_0001_0004_0005_session_rows_append_with_distinct_ulids`.
- [ ] One send adds one row under `<log_root>/sessions/`: tests/contract/test_contract.py:135 `test_session_and_general_log_share_fixed_root`.
- [ ] A query sent four times after three busy responses adds one row: crates/sc-agent/tests/sending.rs:186 `req_sag_0601_busy_resends_unchanged_retry_after_and_exponential_backoff`.
- [ ] A 401 response adds one row whose `model_answered` is null, and a decider stub that answers a `decider-4b-v2.1` request as `decider-2b-v11` adds one row whose `model_answered` is `decider-2b-v11` while the call returns `decision.model_mismatch`: planned in agt-j-2 deliverable J2-9.
- [ ] A batch of 3 through a Run adds 3 link records with `cached: false` to `runs.jsonl` in the same directory: planned in agt-j-2 deliverable J2-5.
- [ ] A cache hit adds no row and one link record with `cached: true`: planned in agt-j-2 deliverable J2-7.

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
