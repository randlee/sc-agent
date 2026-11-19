# Requirement: Logger injection into the Rust API and the wheels

**Status:** Approved  
**Version:** 0.3.1  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-18  
**Owner:** Rand Lee  

---

## REQ-SAG-0008: Logger injection into the Rust API and the wheels

**Status:** Approved  

### Requirement Statement

Definitions: The general logger writes the general log. The session logger writes session logs. Injecting a logger means the caller passes in the logger sc-agent uses. A wheel is the sc-decidr or the sitr Python package. An injection point is sc-agent's Rust API, the sc-decidr wheel or the sitr wheel. Consumer glue is a consumer adapter or a consumer record mapping: code a consumer writes between sc-agent and its own Logger. A Logger's lifecycle is its flush and its shutdown. A logger form is a caller-owned `sc_observability.Logger` or a callable that receives each record. A caller-owned `sc_observability.Logger` is an sc-observability Logger that the caller created through the `sc-observability` Python package, for example with `sc_observability.create_logger`.

**MUST statements:**

- The general logger and the session logger MUST be separate loggers.
- Every injection point MUST let its caller inject the general logger.
- Every injection point MUST let its caller inject the session logger.
- A wheel MUST accept an injected logger given as a caller-owned `sc_observability.Logger`.
- A wheel MUST accept an injected logger given as a callable that receives each record.
- When a logger is not injected, sc-agent MUST use its default logger for that log.
- When a caller injects an `sc_observability.Logger`, sc-agent MUST deliver each record through that Logger's public `log(LogEvent)` API.
- An injected logger MUST replace only the default logger of its own log.
- The `pyproject.toml` of each wheel MUST list the `sc-observability` Python package in its `dependencies`.

**MUST NOT statements:**

- When a caller injects an `sc_observability.Logger`, sc-agent MUST NOT take over that Logger's lifecycle from the caller.
- When a caller injects an `sc_observability.Logger`, record delivery to that Logger MUST NOT require consumer glue.
- When a logger is injected, a record of its log MUST NOT pass through any logger other than the injected logger.

### Rationale

Applications built on sc-agent route logs into their own logging, and a caller that does not care still gets both logs.

### Success Criteria

**Acceptance Criteria:**

- [ ] `Loggers::open` for the `decidr` service with no injected logger writes its general-log records to `<log_root>/logs/decidr.log.jsonl` and its session rows to `<log_root>/sessions/<repo>/<agent_role>/<project>/sessions.jsonl`.
- [ ] `Loggers::open` for the `decidr` service with an injected session Logger sends each session row to that Logger and writes its general-log records to `<log_root>/logs/decidr.log.jsonl`.
- [ ] `Loggers::open` for the `decidr` service with an injected general Logger sends each general-log record to that Logger and writes its session rows to `<log_root>/sessions/<repo>/<agent_role>/<project>/sessions.jsonl`.
- [ ] A Python call to the sc-decidr wheel with an injected session-log callable passes each session row to that callable and writes the general log through the default general logger.
- [ ] A Python call to the sitr wheel with an injected general Logger writes its general-log records to that Logger.
- [ ] In a fresh virtual environment, `pip install sc-decidr` also installs the `sc-observability` Python package, and `pip install sitr` does the same.
- [ ] A Python program that creates one Logger with `sc_observability.create_logger` and injects it as the general logger into an sc-decidr call and a sitr call finds its own records, the sc-decidr records and the sitr records in that Logger's one log file.

### Dependencies

**Related:**

- REQ-SAG-0003
- REQ-SAG-0022
- ADR-SAG-0019
- ADR-SAG-0021
