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

Definitions: A Logger is an sc-observability crate `Logger`. The linked crate is the sc-observability Rust crate compiled into the wheel. The general logger writes the general log. Injecting a Logger means the caller passes in the Logger sc-agent uses. The wheel is the sc-decidr Python package, which holds the decidr and sitr Python APIs (ADR-SAG-0021).

**MUST statements:**

- The Rust API MUST accept an injected general Logger.
- When a Python caller requests a named Logger, the wheel MUST return a Logger built from the linked crate.

**MUST NOT statements:**

- The wheel MUST NOT depend on the `sc-observability` Python package.

### Rationale

The wheel already links the sc-observability crate, so it hands that crate's Logger to Python and receives the same object back: one Logger, one queue, no bridge.

### Success Criteria

**Acceptance Criteria:**

- [ ] Unit test: a Rust caller injects a general Logger with a test sink and finds every general-log record of the call in that sink.
- [ ] Python test: a program obtains a Logger named `t` from the wheel, writes one record, and finds its record in `~/.sc-agent/logs/t.log.jsonl`.
- [ ] Install check: in a fresh virtual environment, `pip install sc-decidr` installs no `sc-observability` package.

### Dependencies

**Related:**

- REQ-SAG-0003
- ADR-SAG-0019
- ADR-SAG-0021
