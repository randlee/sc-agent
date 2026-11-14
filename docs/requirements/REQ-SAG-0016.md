# Requirement: Error details carry the HTTP status, send count and Retry-After as typed fields

**Status:** Proposed  
**Version:** 0.1.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-14  
**Owner:** Rand Lee  

---

## REQ-SAG-0016: Error details carry the HTTP status, send count and Retry-After as typed fields

**Status:** Proposed  

### Requirement Statement

Definitions: An error detail is `ErrorDetail`, the data form of an error that the CLI, the Python binding, session rows and result lines carry. A status-bearing error is one built from an HTTP reply.

**MUST statements:**

- `ErrorDetail` MUST have the optional fields `status` (HTTP status, `u16`), `attempts` (sends made, the first included, `u32`) and `retry_after_ms` (the last reply's valid `Retry-After` in whole milliseconds, rounded up, `u64`).
- Every error variant that holds an HTTP status, a send count or a `Retry-After` MUST set the matching field in its detail. The variants are listed in ADR-SAG-0019 decision 2.
- A variant that wraps a whole lower error MUST carry that error's fields at its own level as well as in its cause.
- Each field MUST be omitted from serialized JSON when it is not set.
- `ErrorId::detail()` MUST remain the only builder of an error's detail, and it MUST take the fields from `ErrorId::fields()`.
- `sc_agent.SagError` MUST expose `status`, `attempts` and `retry_after_ms` attributes equal to its top-level detail's fields, `None` when not set.

**MUST NOT statements:**

- `ErrorDetail` MUST NOT lose `Eq`.

### Rationale

Consumers decide retries, back-off and authentication fixes from the status, send count and `Retry-After`. Today these values exist only inside English message text, so consumers parse prose. `retry_after_ms` is a transport timing hint, not a decision value; REQ-SAG-0007 governs decision values (owner, 2026-11-14).

### Success Criteria

**Acceptance Criteria:**

- [ ] For each variant in ADR-SAG-0019 decision 2, a test builds the error and serializes its detail. In Python, the test reads the exact `status`, `attempts` and `retry_after_ms` values from `SagError` or from the result line's `error`.
- [ ] A `ProviderBusy` wrapped as `decision.http` has the same `status`, `attempts` and `retry_after_ms` at the top level as at the `http.provider_busy` cause.
- [ ] A `Retry-After: 2.0001` reply yields `retry_after_ms == 2001`.
- [ ] Each crate's test that `detail()` equals the value built from the trait methods includes `fields()`.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0002
- REQ-SAG-0007
- REQ-SAG-0601
- REQ-SAG-0602
- NFR-SAG-0002
