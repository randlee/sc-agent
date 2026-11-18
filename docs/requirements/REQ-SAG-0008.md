# Requirement: Logger injection: the app injects its general Logger, defaults otherwise

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-18  
**Owner:** Rand Lee  

---

## REQ-SAG-0008: Logger injection: the app injects its general Logger, defaults otherwise

**Status:** Approved  

### Requirement Statement

Definitions: The general logger writes the general log. The session logger writes session logs. Injecting a logger means the caller passes in the logger sc-agent uses.

**MUST statements:**

- The general logger and the session logger MUST be separate loggers.
- A caller MUST be able to inject the general logger, the session logger, or both.
- When a logger is not injected, sc-agent MUST use its default logger for that log.
- The Python API (`sc-agent-py`) MUST let a caller inject the general logger, the session logger, or both, each as a caller-owned `sc_observability.Logger` or a callable that receives each record; a logger not injected MUST be the default logger.
- The sitr wheel MUST accept an injected general logger in the same way the sc-decidr wheel does.
- The generated consumer client (REQ-SAG-0018) MUST own the general logger: it MUST obtain one `sc_observability.Logger` from sc-observability, named after the client, and inject that same Logger into every sc-decidr and sitr call it makes.
- The client name MUST be the consumer's product name, the same value as its `agent_role` (REQ-SAG-0004), such as `plan-anchor` or `judge-jev`.
- The client's default general log MUST be `<log_root>/logs/<client>/<client>.log.jsonl`, such as `<log_root>/logs/judge-jev/judge-jev.log.jsonl`, and MUST hold the client's own records and the records of every sc-decidr and sitr call and error it made.
- The generated client MUST let the app pass its own `sc_observability.Logger` in place of the default one, and MUST then inject that Logger into every sc-decidr and sitr call.
- The generated client MUST NOT offer a mode in which an sc-decidr or sitr call gets no general logger.
- The session logger MUST stay optionally injectable; with none injected, the generated client's sessions MUST be written to the default session log (REQ-SAG-0005).
- With nothing configured, every log in the ADR-SAG-0021 tables MUST be enabled and written under `<log_root>`, and every sc-decidr and sitr call MUST be logged.

- Injecting a Python Logger MUST forward records into that same Logger through its public `log(LogEvent)` API, without a consumer adapter or consumer record mapping. The caller MUST retain ownership of flush and shutdown. An injected Logger replaces only its corresponding default.

### Rationale

Applications built on sc-agent need to route logs into their own logging, and a caller that does not care must still get both logs.

### Success Criteria

**Acceptance Criteria:**

- [ ] A run with no injected loggers writes both the general log and the session log through the default loggers.
- [ ] A run with an injected session logger writes its session rows to that logger and its general log to the default general logger.
- [ ] A run with an injected general logger writes its general log to that logger and its session rows to the default session logger.
- [ ] A Python call to `sc-agent-py` with an injected session-log callable passes each session row to that callable and writes the general log through the default general logger.
- [ ] A Python call to `sc-agent-py` with an injected general-log callable passes each general-log record to that callable and writes session rows through the default session logger.
- [ ] In judge-jev, one sc-decidr call and one sitr call through the generated client write their records, and the record of a failed sitr call, into `<log_root>/logs/judge-jev/judge-jev.log.jsonl`, and no `decidr.log.jsonl`, `sitr.log.jsonl` or `sc-agent-py.log.jsonl` is written.
- [ ] In plan-anchor, an sc-decidr call through the generated client writes its records into `<log_root>/logs/plan-anchor/plan-anchor.log.jsonl`.
- [ ] With nothing injected and nothing configured, a judge-jev run that makes sc-decidr and sitr calls writes `<log_root>/logs/judge-jev/judge-jev.log.jsonl` and `<log_root>/sessions/<repo>/judge-jev/<project>/sessions.jsonl`, and writes no log outside `<log_root>`.
- [ ] With the app's own Logger passed to the generated client, the sc-decidr and sitr records go to that Logger and no default general log file is written.

### Dependencies

**Related:**

- REQ-SAG-0003