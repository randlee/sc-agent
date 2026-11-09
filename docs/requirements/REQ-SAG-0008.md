# Requirement: Optional logger injection with default loggers

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0008: Optional logger injection with default loggers

**Status:** Draft  

### Requirement Statement

Definitions: The general logger writes the general log. The session logger writes session logs. Injecting a logger means the caller passes in the logger sc-agent uses.

**MUST statements:**

- The general logger and the session logger MUST be separate loggers.
- A caller MUST be able to inject the general logger, the session logger, or both.
- When a logger is not injected, sc-agent MUST use its default logger for that log.

### Rationale

Applications built on sc-agent need to route logs into their own logging, and a caller that does not care must still get both logs.

### Success Criteria

**Acceptance Criteria:**

- [ ] A run with no injected loggers writes both the general log and the session log through the default loggers.
- [ ] A run with an injected session logger writes its session rows to that logger and its general log to the default general logger.
- [ ] A run with an injected general logger writes its general log to that logger and its session rows to the default session logger.

### Dependencies

**Related:**

- REQ-SAG-0003