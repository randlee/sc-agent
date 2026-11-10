# Requirement: Errors are a discriminated union carried from bottom to top

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0002: Errors are a discriminated union carried from bottom to top

**Status:** Approved  

### Requirement Statement

Definitions: A discriminated union is a tagged result type whose variant names the outcome, such as Rust `Result<T, E>` with an error enum. An error id is the stable identifier of one error case. A lower layer is any crate or function an API calls.

**MUST statements:**

- Every public API MUST return a discriminated union of success and error.
- Rust APIs MUST return `Result<T, E>` from the lowest layer to the top-level CLI and PyO3 entry points.
- An error MUST carry the error id and message of every lower-layer error that caused it.
- A layer that changes an error MUST change it only by adding detail.
- Error text received from a provider MUST be kept whole.
- Every error variant MUST have a stable id of the form `<crate>.<variant>`, such as `http.provider_busy`.

**MUST NOT statements:**

- Rust code MUST NOT panic on any input or runtime failure.
- A layer MUST NOT drop or replace a lower-layer error id or message.

### Rationale

Agents and tools act on errors programmatically. A dropped lower-layer id or a panic removes the information needed to decide the next step. This rule applies to every repository the owner maintains.

### Success Criteria

**Acceptance Criteria:**

- [ ] A test that injects a failure at the lowest layer of a call path receives, at the CLI and PyO3 entry points, that failure's error id and message in the returned error.
- [ ] Clippy runs with `panic`, `unwrap_used`, `expect_used` and `indexing_slicing` denied in non-test code and passes.

### Product Applicability

**Applies To:**

- Every crate, the CLI and the PyO3 bindings.