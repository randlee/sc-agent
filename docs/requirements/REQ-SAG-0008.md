# Requirement: Optional logger injection with default loggers

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0008: Optional logger injection with default loggers

**Status:** Approved  

### Requirement Statement

Definitions: The general logger writes the general log. The session logger writes session logs. Injecting a logger means the caller passes in the logger sc-agent uses.

**MUST statements:**

- The general logger and the session logger MUST be separate loggers.
- A caller MUST be able to inject the general logger, the session logger, or both.
- When a logger is not injected, sc-agent MUST use its default logger for that log.
- The Python API (`sc-agent-py`) MUST let a caller inject the general logger, the session logger, or both, each as a caller-owned `sc_observability.Logger` or a callable that receives each record; a logger not injected MUST be the default logger.

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

### Dependencies

**Related:**

- REQ-SAG-0003