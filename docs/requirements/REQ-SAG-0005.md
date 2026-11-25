# Requirement: Session log JSONL layout and row content

**Status:** Approved  
**Version:** 0.7.2  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0005: Session log JSONL layout and row content

**Status:** Approved  

### Requirement Statement

Definitions: `<log_root>` is `~/.sc-agent`, or a CLI's `<CLI>_LOG_ROOT` when set and non-empty (REQ-SAG-0003). JSON is JavaScript Object Notation (RFC 8259). JSONL is newline-delimited JSON: one JSON object per line. A CLI is a command-line interface program. The session directory is `<log_root>/sessions/<repo>/<agent_role>/<project>/`. A session's session directory is the session directory for that session's repo, agent role and project. A query is one decision request sent through `send`, or one request of a `send_batch` call. A session row is the JSON object written for one query. A session event is the sc-observability log event that holds one session row, its wrapper included. 1 MiB is 1,048,576 bytes. A cache hit is defined in REQ-SAG-0703. `model_requested` is the fixed model id of the query's resolved model (REQ-SAG-0102). `provider` is the provider the query was sent to. `model_answered` is the model id the provider's response reported. `usage` is the token usage the provider reported, as `input_tokens` and `output_tokens`. A Run is the context object through which every query is sent (ADR-SAG-0019 decision 14). `run_id` is the ULID that identifies one Run; it is minted in Rust when the Run is created. The run log is the file `runs.jsonl` in the session directory. A link record is the JSON object `{run_id, session_id, cached}` written to the run log for one query. A session sink is defined in REQ-SAG-0003.

**MUST statements:**

- Session logs MUST be written as JSONL.
- Session logs MUST be written in the session directory.
- Every session MUST be appended to the file `sessions.jsonl` in its session directory.
- `decidr` MUST write its session logs in the session directory.
- sc-agent MUST write exactly one session row for every query it sends to a model.
- Every session row MUST carry `schema` with the value `sc-agent-session/2`.
- Every session row MUST carry `model_requested`.
- Every session row MUST carry `provider`.
- Every session row MUST carry `model_answered`.
- `model_answered` MUST be null when no provider response was received.
- Every session row MUST carry `usage`.
- `usage` MUST be null when the provider reported no token usage.
- A session event of at most 1 MiB MUST be written whole, with the full request.
- When a session event is larger than 1 MiB, sc-agent MUST write no row for that query.
- When a session event is larger than 1 MiB, sc-agent MUST return a `log.session_write` error.
- When a session write fails, sc-agent MUST return a `log.session_write` error.
- Every query MUST be sent through a Run.
- Creating a Run MUST mint its `run_id` in Rust.
- `decidr send` MUST create one Run per command run and send every query of that command through it.
- A call through a closed Run MUST return the error value `decision.run_closed`, with category `validation`, and send nothing.
- A query MUST be sent only with a resolved model, so every session row's `provider` and `model_requested` name a provider and a model id.
- For every query it writes a session row for, sc-agent MUST append one link record to the run log with that query's `run_id`, that row's `session_id` and `cached: false`.
- For every cache hit (REQ-SAG-0703), sc-agent MUST append one link record to the run log with that query's `run_id`, the matching row's `session_id` and `cached: true`.
- Link records MUST be written in Rust through a session sink.

**MUST NOT statements:**

- sc-agent MUST NOT offer any setting, argument or environment variable that turns session logging off.
- A cache hit MUST NOT write a session row.
- A session row MUST NOT carry `run_id`.
- sc-agent MUST NOT rotate, prune or delete a run log.
- A session row MUST NOT carry a start time or an end time; its `session_id` ULID carries the time.
- sc-agent MUST NOT truncate a request to make its session event fit.

### Rationale

Directory separation lets tools select a repository, role or project with a path glob without opening files. Every Jev query from every tool lands in the session log, so tool records reference a row by `session_id` instead of copying the query or the response. Runs and sessions are many-to-many: one run uses many sessions, and through cache hits one session serves many runs, so the session row cannot hold the run. The run log holds the link; the link with `cached: false` names the run that created the session.

Owner ruling 2026-11-24 (session log layer): "the software ... should work in layers. When a different model responds than expected, repeating the mistake would be a cache miss because the requested model does not have an entry. Whether you should accept/return the answer is a different question than what is logged. Session log logs what happened. It has no control over a misconfigured computer."

Owner ruling 2026-11-24 (plan review, ADR-SAG-0003): `run_id` is minted in Rust when a Run is created; consumers never mint it.

Owner rulings (2026-11-24), Rand Lee, recorded in `docs/plans/phase-j/audit-trail-log-normalization-SQLite.md`: the session log is written through sc-observability, which accepts events up to 1 MiB; the row carries the model, provider and token usage; the ULID carries the time. Round 3: the session row carries no `run_id`; `send` and `send_batch` take the caller's `run_id`, and sc-agent appends `{run_id, session_id, cached}` to `runs.jsonl` beside `sessions.jsonl`, written the same way and never rotated, on a cache miss and on a cache hit.

### Success Criteria

**Acceptance Criteria:**

- [ ] After a session with `repo` `github.com/randlee/sc-agent`, `agent_role` `rust-developer` and `project` `phase-b`, its rows are in a file in `<log_root>/sessions/github.com/randlee/sc-agent/rust-developer/phase-b/sessions.jsonl`.
- [ ] A `send_batch` of 3 requests against the loopback runner adds exactly 3 rows, each with `schema` `sc-agent-session/2`, `model_requested`, `provider`, `model_answered` and `usage`.
- [ ] A loopback response that reports no token usage gives a row whose `usage` is null.
- [ ] A decider stub that replies with `"model": "decider-2b-v11"` to a `decider-4b-v2.1` request adds one row whose `model_requested` is `decider-4b-v2.1`, whose `model_answered` is `decider-2b-v11` and whose `error` is `decision.model_mismatch`.
- [ ] A request whose session event is just under 1 MiB is written whole; a request whose session event is over 1 MiB returns `log.session_write` and adds no row.
- [ ] A cache hit (REQ-SAG-0703) adds no row.
- [ ] A `send_batch` of 3 requests with `run_id` R against the loopback runner adds 3 link records to `runs.jsonl` in the same session directory, each with `run_id` R, its row's `session_id` and `cached: false`, and none of the 3 session rows has a `run_id` field.
- [ ] A cache hit with `run_id` R2 adds one link record with `run_id` R2, the matching row's `session_id` and `cached: true`.
- [ ] A `send` through a closed Run returns `decision.run_closed` with category `validation` and adds no session row and no link record.
- [ ] Two Runs created in one process have different ULID `run_id`s.
- [ ] After the run log grows past the sc-observability rotation size, the next link record is appended to the same `runs.jsonl`, and no rotated copy of it exists.

### Dependencies

**Related:**

- REQ-SAG-0004
- ADR-SAG-0002
- ADR-SAG-0021
- REQ-SAG-0003
- REQ-SAG-0703