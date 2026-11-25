# Requirement: Errors are a discriminated union carried from bottom to top

**Status:** Approved  
**Version:** 0.3.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-21  
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
- Exception (owner-approved): the API key MUST never appear in an error or log.
- Every error MUST have a stable id.

**MUST NOT statements:**

- Rust code MUST NOT panic on any input or runtime failure.
- A layer MUST NOT drop or replace a lower-layer error id or message.

### Rationale

Agents and tools act on errors programmatically. A dropped lower-layer id or a panic removes the information needed to decide the next step. This rule applies to every repository the owner maintains.

Owner ruling (2026-11-21), Rand Lee: "always return descriminated unions w/ error info"

### Success Criteria

**Acceptance Criteria:**

- [ ] A test that injects a failure at the lowest layer of a call path receives, at the CLI and PyO3 entry points, that failure's error id and message in the returned error.
- [ ] A test in which a provider or local runner echoes the attached key receives an error whose id is unchanged and which contains no copy of the key.
- [ ] Clippy passes with the workspace lint configuration (`[workspace.lints.clippy]` in `Cargo.toml`), which denies panicking constructs in non-test code.

### Product Applicability

**Applies To:**

- Every crate, the CLI and the PyO3 bindings.