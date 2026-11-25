# Requirement: Values passed together are one Rust struct

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-24  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## NFR-SAG-0005: Values passed together are one Rust struct

**Status:** Approved  

### Requirement Statement

Definitions:

- A collection is a set of two or more values that are passed together through two or more call layers, as parameters or as a returned value.
- A call layer is one function that receives the values and passes them, or values derived from them, to another function.
- A parallel parameter list is a collection passed as separate parameters of one function.
- A string-keyed option table is a list or map from option names to values that is looked up by name, such as a list of `(name, value)` pairs or a Python dict of options.
- The PyO3 layer is `crates/sc-agent-py/src/lib.rs`.

**MUST statements:**

- Every collection in the sc-agent workspace MUST be one named Rust struct, one type per collection.
- A Python-facing collection MUST be a `#[pyclass]` with typed fields.

**MUST NOT statements:**

- A collection MUST NOT be passed as a parallel parameter list.
- A function MUST NOT return a tuple of values that its caller passes on together.
- The PyO3 layer MUST NOT look up an option by its name in a string-keyed option table.
- A collection MUST NOT be passed as a `serde_json::Value` object or a Python dict, except where it is the wire body, a session row or a returned result value.

### Rationale

A collection passed as separate parameters, tuples or keyed tables is repeated at every layer it crosses, so each layer can drift from the others; one struct is one owner for the collection (NFR-SAG-0004).

Owner ruling 2026-11-24: any set of variables passed together up or down the call stack is a Rust struct, one type per collection, never parallel parameters, tuples, string-keyed option tables or dicts.

Lead ruling 2026-11-24, pending owner confirmation: the Run's send entry points are methods of the Run (`Run::send`, `Run::batch`), not free functions taking the Run (ADR-SAG-0019 decision 14). A single value (the run's ULID, the cache policy) is not a collection and needs no newtype.

### Success Criteria

**Acceptance Criteria:**

- [ ] `Run::send` and `Run::batch` are methods of the Run struct and take only the per-call values; the logger, cancel flag, wait function and keyring lookup are fields of the Run or of its test-seam struct, not separate parameters.
- [ ] Key resolution takes one struct holding the environment snapshot, the repository root and the keyring lookup; model resolution returns one struct holding the provider and the model id; configuration loading takes one struct holding the local folder, the global folder, the environment snapshot and the project; repository discovery returns one struct holding the root and the identity.
- [ ] The PyO3 layer has no function that looks up an option by name, and no `PyCapsule`.
- [ ] QA reports a new parallel parameter list, collection tuple or string-keyed option table in a sprint's diff as a blocking finding.

### Dependencies

**Related:**

- NFR-SAG-0004
- ADR-SAG-0019
- ADR-SAG-0004
