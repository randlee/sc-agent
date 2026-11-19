# Requirement: Logger injection and the generated client's logger

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-18  
**Owner:** Rand Lee  

---

## REQ-SAG-0008: Logger injection and the generated client's logger

**Status:** Approved  

### Requirement Statement

Definitions: The general logger writes the general log. The session logger writes session logs. Injecting a logger means the caller passes in the logger sc-agent uses. A wheel is the sc-decidr or the sitr Python package. The generated client is the Python module a consumer renders from the sc-decidr wheel's sc-compose template (REQ-SAG-0018). The client name is the consumer's product name and equals its `agent_role` (REQ-SAG-0004), such as `plan-anchor` or `judge-jev`.

**MUST statements:**

- The general logger and the session logger MUST be separate loggers.
- A caller of sc-agent's Rust API or of either wheel MUST be able to inject the general logger, the session logger, or both.
- A wheel MUST accept each injected logger as a caller-owned `sc_observability.Logger` or as a callable that receives each record.
- When a logger is not injected, sc-agent MUST use its default logger for that log.
- An injected `sc_observability.Logger` MUST receive records through its public `log(LogEvent)` API, without a consumer adapter or consumer record mapping, and its caller MUST keep ownership of flush and shutdown.
- An injected logger MUST replace only the default logger of its own log.
- The generated client MUST obtain one `sc_observability.Logger` named after the client and inject that same Logger as the general logger into every sc-decidr call and every sitr call it makes.
- The generated client's Logger MUST write by default to `<log_root>/logs/<client>/<client>.log.jsonl`, and that file MUST hold the client's own records and the records of every sc-decidr and sitr call the client made, including failed calls.
- The generated client MUST accept an app's own `sc_observability.Logger` in place of its default one, and MUST then inject the app's Logger into every sc-decidr call and every sitr call.
- The generated client MUST leave the session logger optional: with no session logger injected, its sessions MUST go to the default session log (REQ-SAG-0005).
- With no logger injected and no log setting configured, the generated client's general log and session log MUST both be written under `<log_root>`, and every sc-decidr call and every sitr call MUST be logged.

**MUST NOT statements:**

- The generated client MUST NOT make an sc-decidr call or a sitr call without a general logger.

### Rationale

Applications built on sc-agent route logs into their own logging, and a caller that does not care still gets both logs. One Logger per client puts an app's records, its decidr calls and its sitr calls in one file, and default logs under `<log_root>` give the owner every log in one place for audit.

### Success Criteria

**Acceptance Criteria:**

- [ ] A run with no injected loggers writes both the general log and the session log through the default loggers.
- [ ] A run with an injected session logger writes its session rows to that logger and its general log to the default general logger.
- [ ] A run with an injected general logger writes its general log to that logger and its session rows to the default session logger.
- [ ] A Python call to the sc-decidr wheel with an injected session-log callable passes each session row to that callable and writes the general log through the default general logger.
- [ ] A Python call to the sitr wheel with an injected general Logger writes its general-log records to that Logger.
- [ ] In judge-jev with nothing injected or configured, one sc-decidr call, one sitr call and one failed sitr call through the generated client write their records into `<log_root>/logs/judge-jev/judge-jev.log.jsonl`, the session into `<log_root>/sessions/<repo>/judge-jev/<project>/sessions.jsonl`, and no `decidr.log.jsonl`, `sitr.log.jsonl` or `sc-agent-py.log.jsonl`.
- [ ] In plan-anchor, an sc-decidr call through the generated client writes its records into `<log_root>/logs/plan-anchor/plan-anchor.log.jsonl`.
- [ ] With an app's own Logger passed to the generated client, the sc-decidr and sitr records go to that Logger and no default general log file is written.

### Dependencies

**Related:**

- REQ-SAG-0003
- REQ-SAG-0004
- REQ-SAG-0005
- REQ-SAG-0018
- ADR-SAG-0019
- ADR-SAG-0021
