# Requirement: Error details carry the HTTP status, send count and Retry-After as typed fields

**Status:** Proposed  
**Version:** 0.2.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-14  
**Owner:** Rand Lee  

---

## REQ-SAG-0016: Error details carry the HTTP status, send count and Retry-After as typed fields

**Status:** Proposed  

### Requirement Statement

Definitions:

- An error detail is `ErrorDetail`, the data form of an error that the CLI, the Python binding, session rows and result lines carry.
- A leaf variant is an error variant that holds data of its own.
- A wrapper is a variant whose cause is a whole lower error.

**MUST statements:**

- `ErrorDetail` MUST have these optional fields:
  - `status`, the HTTP status, a `u16`;
  - `attempts`, the sends made with the first included, a `u32`;
  - `retry_after_ms`, the last reply's valid `Retry-After` in whole milliseconds, a `u64`.
- Each field MUST be omitted from serialized JSON when it is not set.
- Every leaf variant listed in ADR-SAG-0019 decision 2 MUST set the fields that table gives it.
- `ErrorId::detail()` MUST remain the only builder of a detail. It MUST take each field from the variant's own accessor and, when that is `None`, from the cause's detail.
- `retry_after_ms` MUST be computed from the parsed integer seconds and nanoseconds:
  - as the ceiling in milliseconds;
  - saturating at `u64::MAX`;
  - without clamping;
  - with `0` giving `0`.
- `ErrorDetail::map_text` MUST rewrite id and message text throughout the chain and keep `recovery` and every field.
- `sc_agent.SagError` MUST be an exception class whose args are `(id, message, detail_json)`. It MUST expose:
  - the read-only attributes `id`, `message`, `detail`, `status`, `attempts`, `retry_after_ms`, `category` and `retryable`;
  - the field values taken from its top-level detail.
- These attributes MUST be present on a `SagError` built in Python and on one that has been pickled and restored.

**MUST NOT statements:**

- `ErrorDetail` MUST NOT lose `Eq`.
- A consumer-facing error value MUST NOT require parsing message text to obtain a status, send count or Retry-After.

### Rationale

Consumers decide whether to retry or back off, and how to fix authentication, from the status, the send count and the Retry-After. Today most of these values exist only in English message text. `retry_after_ms` is a transport timing hint; REQ-SAG-0007 governs decision values.

### Success Criteria

**Acceptance Criteria:**

- [ ] For each leaf row in ADR-SAG-0019 decision 2, a Rust test asserts the detail's fields. A Python test reads the same values through `SagError` or the result line's `error`.
- [ ] `decision.http` over `ProviderBusy`, and `decision.backend` over a 401 `llm.api_status`, carry the cause's fields at the top level.
- [ ] `http.response_too_large` with status 0 has `status` `None`.
- [ ] `decision.cancelled` carries `attempts` equal to its sends.
- [ ] `RetryAfter::parse` gives these `retry_after_ms` values:

  | Header | `retry_after_ms` |
  | --- | --- |
  | `1.1` | 1100 |
  | `2.0001` | 2001 |
  | `0` | 0 |
  | `0.0000001` | 1 |
  | 32 nines | `u64::MAX` |
  | a malformed header | `None` |
- [ ] `map_text` keeps `status`, `attempts`, `retry_after_ms` and `recovery`.
- [ ] `SagError("x", "y", "{}")` has `status` `None` and `category` `"other"`.
- [ ] A pickle round trip of a `SagError` keeps its `status`.
- [ ] Each crate's test that `detail()` equals the value built from the accessors covers all three fields and the copy-up.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0002
- REQ-SAG-0007
- REQ-SAG-0601
- REQ-SAG-0602
- NFR-SAG-0002
