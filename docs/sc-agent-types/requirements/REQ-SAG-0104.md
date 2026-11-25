# Requirement: Cancel token

**Status:** Approved  
**Version:** 0.3.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-25  
**Owner:** Rand Lee  

---

## REQ-SAG-0104: Cancel token

**Status:** Approved  

### Requirement Statement

Definitions: `Cancel` is the Rust type in the `sc-agent` crate that carries one cancellation signal; code cancels it, and every operation holding it stops. An operation is a call `sc-agent` is running. An operation stops when it returns the `decision.cancelled` error without finishing its work. A crate is a Rust package in this repository. A command-line tool built on `sc-agent` is a binary in this repository that calls the `sc-agent` crate; today `decidr` and `sitr`. PyO3 is the Rust library that builds the `sc_agent` Python module from Rust code; the PyO3 bindings are that module. A user interrupts a command-line tool by pressing Ctrl-C or sending it SIGTERM.

**MUST statements:**

- `sc-agent` MUST provide a `Cancel` type.
- Cancelling a `Cancel` MUST stop an operation that holds it and runs in another crate.
- A command-line tool built on `sc-agent` MUST stop its running operation when the user interrupts it.
- The PyO3 bindings MUST let Python code stop a running operation.

### Rationale

Every crate that runs an `sc-agent` call must agree on one cancellation signal, so a cancel from a command-line tool or from Python reaches every crate doing the work.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] A unit test creates a `Cancel` in one crate, cancels it, and shows an operation in another crate stops.
- [ ] A CLI run interrupted by the user stops its running operation and exits.
- [ ] A Python test cancels a running operation through the bindings and shows it stops.