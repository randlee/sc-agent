# Requirement: Error details carry the HTTP status, send count and Retry-After as typed fields

**Status:** Approved  
**Version:** 0.6.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-20  
**Owner:** Rand Lee  

---

## REQ-SAG-0016: Error details carry the HTTP status, send count and Retry-After as typed fields

**Status:** Approved  

### Requirement Statement

Definitions:

- An error detail is `ErrorDetail`, the data form of an error that the CLI, the Python binding, session rows and result lines carry.
- A leaf variant is an error variant that holds data of its own.
- A wrapper is a variant whose cause is a whole lower error.
- The typed fields are four optional fields of an error detail:
  - `status`, the HTTP status, a `u16`;
  - `attempts`, the sends made with the first included, a `u32`;
  - `retry_after_ms`, the last reply's valid `Retry-After` in whole milliseconds, a `u64`;
  - `category`, the error category that REQ-SAG-0017 assigns.
- The parsed Retry-After is the integer seconds and nanoseconds that `RetryAfter::parse` holds for a valid `Retry-After` header.
- The SagError attributes are `id`, `message`, `detail`, `status`, `attempts`, `retry_after_ms`, `category` and `retryable`. The last five take their values from the error's top-level detail; `category` is `"other"` when that detail has none.
- The leaf field table gives the typed fields each leaf variant sets (`—` means not set):

  | Id | status | attempts | retry_after_ms |
  | --- | --- | --- | --- |
  | `http.provider_busy` | the last reply's status | its send count | the last reply's `retry_after_ms` |
  | `http.quota_exhausted` | the reply's status | its send count | the reply's `retry_after_ms` |
  | `http.network` | — | its send count | — |
  | `http.response_too_large` | the reply's status, not set when 0 | — | — |
  | `llm.api_status` | the reply's status | — | the reply's `retry_after_ms` |
  | `decision.api_bad_request` | 400 | — | — |
  | `decision.api_validation` | 422 | — | — |
  | `decision.api_status` | the reply's status | — | — |
  | `decision.response_parse` | the reply's status | — | — |
  | `local.probe_status` | the reply's status | — | — |
  | `decision.cancelled` | — | its send count | — |

**MUST statements:**

- `ErrorDetail` MUST have the typed fields.
- A typed field that is not set MUST be omitted from serialized JSON.
- Each leaf variant in the leaf field table MUST set the typed fields that the table gives it.
- `ErrorId::detail()` MUST be the only builder of an error detail.
- A wrapper's detail MUST carry the typed fields of its cause's detail.
- A leaf variant's detail MUST carry the leaf variant's own typed fields.
- `retry_after_ms` MUST be the parsed Retry-After rounded up to whole milliseconds.
- A `retry_after_ms` too large for a `u64` MUST be `u64::MAX`.
- The sc-decidr wheel MUST return a failed request as an error value carrying the typed fields, and MUST NOT raise an exception for it.
- An exception raised in Python while a wheel call runs, including one raised by a caller's `on_result` callback, MUST be caught and returned as an error value.

**MUST NOT statements:**

- `ErrorDetail` MUST NOT lose `Eq`.
- `retry_after_ms` MUST NOT be clamped.
- A consumer MUST NOT need to parse message text to obtain a status, send count or Retry-After.

### Rationale

Consumers decide whether to retry or back off, and how to fix authentication, from the status, the send count and the Retry-After. Without typed fields these values exist only in English message text. `retry_after_ms` is a transport timing hint; REQ-SAG-0007 governs decision values.

The 0.1.0 error-JSON compatibility rule is withdrawn (2026-11-20), Rand Lee: "We are still using python, schema isn't locked yet. I haven't mentioned locking schema yet. I have tools to do this that aren't on this repo. No hand built checks."

Owner ruling (2026-11-20), Rand Lee: nothing raises; throwing exceptions is prohibited on the repo.

Owner ruling (2026-11-20), Rand Lee: "Python exceptions need to be caught (return error) so app doesn't crash."

### Success Criteria

**Acceptance Criteria:**

- [ ] For each row of the leaf field table, a Rust test asserts the detail's fields. A Python test reads the same values through `SagError` or the result line's `error`.
- [ ] `decision.http` over `ProviderBusy`, and `decision.backend` over a 401 `llm.api_status`, carry the cause's fields, `category` included, at the top level.
- [ ] A leaf reports its own fields and its own `category`.
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
- [ ] `SagError("x", "y", "{}")` has `status` `None` and `category` `"other"`.
- [ ] A pickle round trip of a `SagError` keeps its `status`.
- [ ] Each crate's test that `detail()` equals the value built from the accessors covers all four fields and the copy-up.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0002
- REQ-SAG-0007
- REQ-SAG-0017
- REQ-SAG-0601
- REQ-SAG-0602
