# Requirement: Logger injection into the Rust API and the wheel

**Status:** Approved  
**Version:** 0.4.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-22  
**Owner:** Rand Lee  

---

## REQ-SAG-0008: Logger injection into the Rust API and the wheel

**Status:** Approved  

### Requirement Statement

Definitions: A Logger is an sc-observability crate `Logger`. The general logger writes the general log. The session logger writes session logs. Injecting a Logger means the caller passes in the Logger sc-agent uses. The entry point is a CLI's `main` or a Python app's generated client (ADR-SAG-0021). The wheel is the sc-decidr Python package, which holds the decidr and sitr Python APIs (ADR-SAG-0021). The default Logger of a log is the sc-observability Logger that writes that log's file at the path ADR-SAG-0021 gives.

**MUST statements:**

- The general logger and the session logger MUST be separate Loggers.
- The Rust API MUST let its caller inject the general logger.
- The Rust API MUST let its caller inject the session logger.
- The wheel MUST return, to a Python caller that names it, a Logger built from the sc-observability crate the wheel links.
- The wheel MUST accept a Logger it returned as the injected general logger of every decidr and sitr call.
- When a Logger is injected, every record of its log MUST be written through that Logger.
- When no Logger is injected for a log, sc-agent MUST write that log through its default Logger.

**MUST NOT statements:**

- When a Logger is injected, sc-agent MUST NOT build another Logger for that log.
- sc-agent MUST NOT forward records to Python `logging` or to a Python callable.
- The wheel MUST NOT depend on the `sc-observability` Python package.
- Library code MUST NOT close an injected Logger; its entry point closes it.

### Rationale

The entry point owns the process's one Logger (ADR-SAG-0021). The wheel already links the sc-observability crate, so it hands that crate's Logger to Python and receives the same object back: one Logger, one queue, no bridge.

### Success Criteria

**Acceptance Criteria:**

- [ ] The `decidr` CLI with no injected Logger writes its general log to `~/.sc-agent/logs/decidr.log.jsonl` and its session rows under `~/.sc-agent/sessions/`.
- [ ] A Rust caller that injects a general Logger with a test sink finds every general-log record of the call in that sink and no `decidr.log.jsonl` written.
- [ ] A Python program obtains a Logger named `t` from the wheel, writes one record to it, injects it into one sc-decidr call, and finds its own record and the call's records in `~/.sc-agent/logs/t.log.jsonl`.
- [ ] In a fresh virtual environment, `pip install sc-decidr` installs no `sc-observability` package.

### Dependencies

**Related:**

- REQ-SAG-0003
- REQ-SAG-0022
- ADR-SAG-0019
- ADR-SAG-0021
