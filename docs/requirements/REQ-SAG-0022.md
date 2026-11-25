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

Definitions: The generated client is the Python module a consumer renders from the sc-decidr wheel's sc-compose template (REQ-SAG-0018). The client name is the consumer's product name, a constant in the generated client, and equals its `agent_role` (REQ-SAG-0004), such as `plan-anchor` or `judge-jev`. The client Logger is the Logger the generated client obtains from the wheel under the client name (REQ-SAG-0008). A wheel call is a call the generated client makes to the sc-decidr wheel's decidr or sitr API. `<CLIENT>_LOG_ROOT` is the environment variable named by the client name uppercased with hyphens replaced by underscores. The general logger is the Logger a wheel call writes its records to (REQ-SAG-0008). The default session files are the session files REQ-SAG-0005 names for an agent role. The session-row reader is the wheel function that returns session rows. A log-root option is a setting or argument that chooses the folder logs are written under.

**MUST statements:**

- The generated client MUST obtain exactly one client Logger per process.
- The generated client MUST inject the client Logger as the general logger into every wheel call.
- With nothing injected by the app, the client Logger MUST write to `~/.sc-agent/logs/<client name>.log.jsonl`.
- When an app gives the generated client its own Logger, the generated client MUST inject the app's Logger as the general logger into every wheel call.
- The generated client MUST leave session rows to the default session files (REQ-SAG-0005).
- The generated client MUST read session rows only through the session-row reader.
- When `<CLIENT>_LOG_ROOT` is set and not empty, the generated client MUST use that folder in place of `~/.sc-agent/` for its logs.

**MUST NOT statements:**

- The generated client MUST NOT have a log-root option.
- The generated client MUST NOT make a wheel call without a general logger.

### Rationale

The generated client does not inject a session Logger.

One Logger per client puts an app's records and all its decidr and sitr calls in one file, and default logs under `~/.sc-agent/` give the owner every log in one place for audit.

### Success Criteria

**Acceptance Criteria:**

- [ ] With nothing injected, judge-jev's generated client makes one sc-decidr call, one sitr call and one failed sitr call; all three calls' records are in `~/.sc-agent/logs/judge-jev.log.jsonl`, the session rows are in `~/.sc-agent/sessions/<repo>/judge-jev/<project>/sessions.jsonl`, and no `decidr.log.jsonl` or `sitr.log.jsonl` is written.
- [ ] With nothing injected, plan-anchor's generated client makes one sc-decidr call and its records are in `~/.sc-agent/logs/plan-anchor.log.jsonl`.
- [ ] A unit test sets `<CLIENT>_LOG_ROOT` to a test folder, makes one sc-decidr call, and finds the general log and session rows under that folder and nothing under `~/.sc-agent/`.
- [ ] A unit test makes one sitr call through a generated client and finds its session row in the default session files for the client name.
- [ ] A unit test wraps every wheel call of a generated client and finds a general logger passed to each.
- [ ] A scan of both rendered generated clients finds no session-log path literal.
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
