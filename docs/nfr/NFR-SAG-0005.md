# Requirement: Values passed together are one Rust struct

**Status:** Approved  
**Version:** 0.4.0  
**Created:** 2026-11-24  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## NFR-SAG-0005: Values passed together are one Rust struct

**Status:** Approved  

### Requirement Statement

Definitions:

- A collection is two or more values that are passed together through two or more functions, as parameters or as a returned value. A free-form value is not a collection: the request text (REQ-SAG-0703), a session row (REQ-SAG-0005), a returned result value, or a general-log event's caller fields.
- The sc-agent workspace is sc-agent's Rust crates, the sc-decidr wheel and the wheel's Python surface; consumer code, including generated consumer clients and the tools that use them, is not part of it.

**MUST statements:**

- Every collection in the sc-agent workspace MUST be passed as a single named Rust struct.
- A collection passed between Python and Rust MUST be a `#[pyclass]` struct.

### Rationale

A collection passed as separate parameters, tuples or keyed tables is repeated at every layer it crosses, so each layer can drift from the others; one struct is one owner for the collection (NFR-SAG-0004). A general-log event's caller fields have keys only the caller knows, so they cannot be a struct.

### Success Criteria

**Acceptance Criteria:**

- [ ] `git grep -n PyCapsule crates/sc-agent-py/src` prints nothing. Today it prints `lib.rs:3`, `lib.rs:73` and `lib.rs:91`, because the PyO3 layer is still pre-phase-j.
- [ ] No function in crates/sc-agent/src returns a tuple or takes a parallel parameter list: planned in agt-j-1 deliverable J1-3.
- [ ] The Python `Run`, `Model`, `Logger` and `TestSeams` are `#[pyclass]` types: planned in agt-j-1 deliverable J1-12.

### Dependencies

**Related:**

- NFR-SAG-0004
- ADR-SAG-0019
- ADR-SAG-0004
