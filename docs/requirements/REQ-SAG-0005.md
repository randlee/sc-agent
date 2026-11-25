# Requirement: Session log JSONL layout and row content

**Status:** Approved  
**Version:** 0.4.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0005: Session log JSONL layout and row content

**Status:** Approved  

### Requirement Statement

Definitions: `<log_root>` is `~/.sc-agent`, or a CLI's `<CLI>_LOG_ROOT` when set (REQ-SAG-0003). JSON is JavaScript Object Notation (RFC 8259). JSONL is newline-delimited JSON: one JSON object per line. A CLI is a command-line interface program. The session directory is `<log_root>/sessions/<repo>/<agent_role>/<project>/`. A session's session directory is the session directory for that session's repo, agent role and project. A query is one decision request sent through `send`, or one request of a `send_batch` call. A session row is the JSON object written for one query. A session event is the sc-observability log event that holds one session row, its wrapper included. 1 MiB is 1,048,576 bytes. A cache hit is defined in REQ-SAG-0703. `model_requested` is the resolved model id the query asked for. `provider` is the provider the query was sent to. `model_answered` is the model id the provider's response reported. `usage` is the token usage the provider reported, as `input_tokens` and `output_tokens`.

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

**MUST NOT statements:**

- sc-agent MUST NOT offer any setting, argument or environment variable that turns session logging off.
- A cache hit MUST NOT write a session row.
- A session row MUST NOT carry a start time or an end time; its `session_id` ULID carries the time.
- sc-agent MUST NOT truncate a request to make its session event fit.

### Rationale

Directory separation lets tools select a repository, role or project with a path glob without opening files. Every Jev query from every tool lands in the session log, so tool records reference a row by `session_id` instead of copying the query or the response.

Owner rulings (2026-11-24), Rand Lee, recorded in `docs/plans/phase-j/audit-trail-log-normalization-SQLite.md`: the session log is written through sc-observability, which accepts events up to 1 MiB; the row carries the model, provider and token usage; the ULID carries the time.

### Success Criteria

**Acceptance Criteria:**

- [ ] After a session with `repo` `github.com/randlee/sc-agent`, `agent_role` `rust-developer` and `project` `phase-b`, its rows are in a file in `<log_root>/sessions/github.com/randlee/sc-agent/rust-developer/phase-b/sessions.jsonl`.
- [ ] A `send_batch` of 3 requests against the loopback runner adds exactly 3 rows, each with `schema` `sc-agent-session/2`, `model_requested`, `provider`, `model_answered` and `usage`.
- [ ] A loopback response that reports no token usage gives a row whose `usage` is null.
- [ ] A request whose session event is just under 1 MiB is written whole; a request whose session event is over 1 MiB returns `log.session_write` and adds no row.
- [ ] A cache hit (REQ-SAG-0703) adds no row.

### Dependencies

**Related:**

- REQ-SAG-0004
- ADR-SAG-0002
- ADR-SAG-0021
- REQ-SAG-0003
- REQ-SAG-0703