# Requirement: Logger injection into the Rust API and the wheel

**Status:** Approved  
**Version:** 0.6.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0008: Logger injection into the Rust API and the wheel

**Status:** Approved  

### Requirement Statement

Definitions: A Logger is an sc-observability crate `Logger`. The general logger writes the general log. Session rows are written in Rust through sc-observability to the default session files (ADR-SAG-0021, REQ-SAG-0003). The default session files are the JSONL files under `~/.sc-agent/sessions/`. Injecting a Logger means the caller passes in the Logger sc-agent uses. The entry point is a CLI's `main` or a Python app's generated client (ADR-SAG-0021). The wheel is the sc-decidr Python package, which holds the decidr and sitr Python APIs (ADR-SAG-0021). The default general Logger is the sc-observability Logger that writes the general log at the path ADR-SAG-0021 gives. A Python callable is any Python object that can be called, such as a function, method, lambda or object with `__call__`.

**MUST statements:**

- sc-agent MUST write session rows to the default session files.
- The Rust API MUST accept an injected general Logger.
- When a Python caller requests a named Logger, the wheel MUST return a Logger built from the sc-observability crate the wheel links.
- Every decidr and sitr call in the wheel MUST accept, as its injected general Logger, a Logger the wheel returned.
- When a general Logger is injected, sc-agent MUST write every general-log record of the call through that Logger.
- When no general Logger is injected, sc-agent MUST write the general log through the default general Logger.
- When a general Logger is injected, each sc-agent call MUST flush that Logger before it returns.

**MUST NOT statements:**

- sc-agent MUST NOT write session rows through the general Logger.
- When a general Logger is injected, sc-agent MUST NOT build another general Logger.
- sc-agent MUST NOT forward log records to Python `logging`.
- sc-agent MUST NOT forward log records to a Python callable.
- The wheel MUST NOT depend on the `sc-observability` Python package.
- sc-agent library code MUST NOT close an injected Logger.

### Rationale

The entry point owns the process's one Logger (ADR-SAG-0021). The wheel already links the sc-observability crate, so it hands that crate's Logger to Python and receives the same object back: one Logger, one queue, no bridge. Session rows stay out of the general log so every tool's session log is the same (written in Rust through sc-observability). The caller owns an injected Logger's lifetime, so library code flushes it but never closes it.

### Success Criteria

**Acceptance Criteria:**

- [ ] CLI run: `decidr` with no injected Logger writes its session rows under `~/.sc-agent/sessions/`.
- [ ] CLI run: `decidr` with no injected Logger writes its general log to `~/.sc-agent/logs/decidr.log.jsonl`.
- [ ] Unit test: a Rust caller injects a general Logger with a test sink and finds every general-log record of the call in that sink, no `decidr.log.jsonl` written, and the sink flushed and still open after the call.
- [ ] Unit test: a Rust call with an injected general Logger on a test sink writes one session row, and the test sink holds no session row.
- [ ] Python test: a program obtains a Logger named `t` from the wheel, writes one record, injects it into one decidr call and one sitr call, and finds its record and the calls' records in `~/.sc-agent/logs/t.log.jsonl`.
- [ ] Python test: with a `logging` handler and no callable registered, one sc-decidr call delivers no record to Python `logging`.
- [ ] Install check: in a fresh virtual environment, `pip install sc-decidr` installs no `sc-observability` package.

### Dependencies

**Related:**

- REQ-SAG-0003
- REQ-SAG-0022
- ADR-SAG-0019
- ADR-SAG-0021
