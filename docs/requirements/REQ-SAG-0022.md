# Requirement: The generated client's general logger

**Status:** Approved  
**Version:** 0.2.1  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-18  
**Owner:** Rand Lee  

---

## REQ-SAG-0022: The generated client's general logger

**Status:** Approved  

### Requirement Statement

Definitions: The general logger writes the general log. The session logger writes session logs. Injecting a logger means the caller passes in the logger sc-agent uses. A wheel is the sc-decidr or the sitr Python package. The generated client is the Python module a consumer renders from the sc-decidr wheel's sc-compose template (REQ-SAG-0018). A wheel call is a call to the sc-decidr wheel or to the sitr wheel. The client Logger is the `sc_observability.Logger` named after the client. The client log file is `<log_root>/logs/<client>.log.jsonl`. A client record is a record the generated client writes itself. A wheel-call record is a record written by a wheel call the generated client made, including a failed wheel call. The generated client is unconfigured when no logger is injected and no log setting is configured. The default session log is the session log REQ-SAG-0005 defines. The client name is the consumer's product name and equals its `agent_role` (REQ-SAG-0004), such as `plan-anchor` or `judge-jev`.

**MUST statements:**

- The generated client MUST obtain exactly one client Logger.
- The generated client MUST inject its client Logger as the general logger into every wheel call it makes.
- When no app Logger is given, the generated client's client Logger MUST write to the client log file.
- When no app Logger is given, the client Logger MUST write every client record to the client log file.
- When the client Logger writes to the client log file, that file MUST hold every wheel-call record.
- The generated client MUST accept an app's own `sc_observability.Logger` in place of its client Logger.
- When given an app's own `sc_observability.Logger`, the generated client MUST inject the app's Logger into every wheel call.
- The generated client MUST leave the session logger optional.
- When no session logger is injected, the generated client MUST write its sessions to the default session log.
- When the generated client is unconfigured, it MUST write its general log under `<log_root>`.
- When the generated client is unconfigured, it MUST write its session log under `<log_root>`.
- When the generated client is unconfigured, it MUST log every wheel call it makes.

**MUST NOT statements:**

- When the generated client has no general logger, it MUST NOT make a wheel call.

### Rationale

One Logger per client puts an app's records, its decidr calls and its sitr calls in one file, and default logs under `<log_root>` give the owner every log in one place for audit.

### Success Criteria

**Acceptance Criteria:**

- [ ] A Python test imports judge-jev's generated client with nothing injected or configured and makes one sc-decidr call, one sitr call and one failed sitr call through it; it then finds the records of all three calls in `<log_root>/logs/judge-jev.log.jsonl`, the session rows in `<log_root>/sessions/<repo>/judge-jev/<project>/sessions.jsonl`, and no `decidr.log.jsonl`, `sitr.log.jsonl` or `sc-agent-py.log.jsonl`.
- [ ] A Python test imports plan-anchor's generated client, makes one sc-decidr call through it, and finds that call's records in `<log_root>/logs/plan-anchor.log.jsonl`.
- [ ] A Python test passes its own `sc_observability.Logger` to a generated client, makes one sc-decidr call and one sitr call, and finds both calls' records in that Logger and no default general log file written.

### Dependencies

**Related:**

- REQ-SAG-0004
- REQ-SAG-0005
- REQ-SAG-0008
- REQ-SAG-0018
- ADR-SAG-0019
- ADR-SAG-0021
