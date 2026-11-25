# Requirement: The generated client owns its Logger

**Status:** Approved  
**Version:** 0.4.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0022: The generated client owns its Logger

**Status:** Approved  

### Requirement Statement

Definitions: The generated client is the Python module a consumer renders from the sc-decidr wheel's sc-compose template (REQ-SAG-0018). The client name is the consumer's product name, a constant in the generated client, and equals its `agent_role` (REQ-SAG-0004), such as `plan-anchor` or `judge-jev`. The client Logger is the Logger the generated client obtains from the wheel under the client name (REQ-SAG-0008). A wheel call is a call the generated client makes to the sc-decidr wheel's decidr or sitr API. A Run is the wheel's `sc_agent.Run` (REQ-SAG-0018). The session-row reader is the wheel function `sc_agent.session_rows(session_ids, *, agent_role, project=None)`, which returns, as an ok value, a map from each requested `session_id` to its session row, read from the session logs of that agent role under the log root of that agent role.

**MUST statements:**

- The generated client MUST obtain exactly one client Logger per process.
- The generated client MUST give the client Logger to every Run it creates and to every sitr call.
- With nothing injected by the app, the client Logger MUST write to `~/.sc-agent/logs/<client name>.log.jsonl`.
- The generated client MUST accept an app's Logger in place of the client Logger and give it to every Run and every sitr call.
- The generated client MUST read session rows only through the session-row reader.
- The session-row reader MUST find the log root with the same function the wheel uses to write session rows: `<CLIENT>_LOG_ROOT` for the agent role, else `~/.sc-agent`.
- When a requested `session_id` has no row, the session-row reader MUST return the error value `session.not_found`, with category `validation`, naming the missing ids.
- When a session log cannot be read, the session-row reader MUST return the error value `session.unreadable`, with category `log`, naming the file.
- Sessions MUST go to the default session files (REQ-SAG-0005); the generated client does not inject a session Logger.

**MUST NOT statements:**

- The generated client MUST NOT have a log-root setting or argument. `<CLIENT>_LOG_ROOT`, with the client name uppercased and hyphens replaced by underscores, is the test environment override for both its general log and session rows; empty or absent uses `~/.sc-agent/`.
- The generated client MUST NOT make a wheel call without a general logger.
- The session-row reader MUST NOT take a log-root argument.
- The generated client MUST NOT compute a session-log path or parse a session-log file.

### Rationale

One Logger per client puts an app's records and all its decidr and sitr calls in one file, and default logs under `~/.sc-agent/` give the owner every log in one place for audit.

Owner ruling 2026-11-24 (plan review, ADR-SAG-0003): the wheel owns the session-row reader; consumers never re-implement the session-log layout, and generated clients have no log-root argument (NFR-SAG-0004).

### Success Criteria

**Acceptance Criteria:**

- [ ] With nothing injected, judge-jev's generated client makes one sc-decidr call, one sitr call and one failed sitr call; all three calls' records are in `~/.sc-agent/logs/judge-jev.log.jsonl`, the session rows are in `~/.sc-agent/sessions/<repo>/judge-jev/<project>/sessions.jsonl`, and no `decidr.log.jsonl` or `sitr.log.jsonl` is written.
- [ ] With nothing injected, plan-anchor's generated client makes one sc-decidr call and its records are in `~/.sc-agent/logs/plan-anchor.log.jsonl`.
- [ ] With `<CLIENT>_LOG_ROOT` set to a test folder holding a session log, `session_rows` returns exactly the requested rows; a missing id returns `session.not_found` with category `validation` and the missing id; an unreadable session log returns `session.unreadable` with category `log`.
- [ ] `session_rows` has no log-root parameter, and a scan of both rendered modules finds no session-log path literal.
- [ ] A test injects its own Logger into a generated client, makes one sc-decidr call and one sitr call, and finds both calls' records in that Logger and no default general log written.

### Dependencies

**Related:**

- REQ-SAG-0004
- REQ-SAG-0005
- REQ-SAG-0008
- REQ-SAG-0018
- ADR-SAG-0019
- ADR-SAG-0021
- NFR-SAG-0004
