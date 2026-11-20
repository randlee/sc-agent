# Requirement: Logger injection into the Rust API and the wheel

**Status:** Proposed  
**Version:** 0.4.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-19  
**Owner:** Rand Lee  

---

## REQ-SAG-0008: Logger injection into the Rust API and the wheel

**Status:** Proposed  

### Requirement Statement

Definitions: A Logger is an sc-observability crate `Logger`. The general logger writes the general log. The session logger writes session logs. Injecting a Logger means the caller passes in the Logger sc-agent uses. The sc-decidr and sitr wheels are separately compiled modules. The sc-decidr wheel owns the client Logger and its callable handle (ADR-SAG-0021). The default Logger of a log is the sc-observability Logger that writes that log's file at the path ADR-SAG-0021 gives.

**MUST statements:**

- The general logger and the session logger MUST be separate Loggers.
- The Rust API MUST let its caller inject the general logger.
- The Rust API MUST let its caller inject the session logger.
- The sc-decidr wheel MUST open a client-named Rust Logger and return its callable handle.
- The decidr and sitr wheel calls MUST accept the handle to the same client Logger.
- When a Logger is injected, every record of its log MUST be written through that Logger.
- When no Logger is injected for a log, sc-agent MUST write that log through its default Logger.

**MUST NOT statements:**

- When a Logger is injected, sc-agent MUST NOT build another Logger for that log.
- sc-agent MUST NOT forward records to Python `logging`, a Python-built logger, or a Python sc-observability Logger. The sitr wheel MUST forward records to the sc-decidr-owned callable handle.
- The wheel MUST NOT depend on the `sc-observability` Python package.
- The Python API MUST NOT allow session-logger injection.

### Rationale

The sc-decidr wheel owns the Rust Logger; its callable handle carries sitr records across the separate compiled modules to the same writer.

### Success Criteria

**Acceptance Criteria:**

- [ ] The `decidr` CLI with no injected Logger writes its general log to `~/.sc-agent/logs/decidr.log.jsonl` and its session rows under `~/.sc-agent/sessions/`.
- [ ] A Rust caller that injects a general Logger with a test sink finds every general-log record of the call in that sink and no `decidr.log.jsonl` written.
- [ ] A generated client obtains the sc-decidr-owned handle named `t`, passes it to sc-decidr and sitr, and finds both calls' records in `~/.sc-agent/logs/t.log.jsonl`.
- [ ] In a fresh virtual environment, `pip install sc-decidr` installs no `sc-observability` package.

### Dependencies

**Related:**

- REQ-SAG-0003
- REQ-SAG-0022
- ADR-SAG-0019
- ADR-SAG-0021
