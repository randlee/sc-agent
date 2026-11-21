# Requirement: Error details carry the HTTP status, send count and Retry-After as typed fields

**Status:** Approved  
**Version:** 0.7.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-21  
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
  - `status`, the HTTP status;
  - `attempts`, the sends made with the first included;
  - `retry_after`, the last reply's valid `Retry-After`, a Duration;
  - `category`, the error category that REQ-SAG-0017 assigns.

**MUST statements:**

- `ErrorDetail` MUST have the typed fields.
- A typed field that is not set MUST be omitted from serialized JSON.
- A busy error MUST carry the HTTP status, the number of sends and the last Retry-After (a Duration).
- A wrapper's detail MUST carry the typed fields of its cause's detail.
- A leaf variant's detail MUST carry the leaf variant's own typed fields.
- The sc-decidr wheel MUST return a failed request as an error value carrying the typed fields, and MUST NOT raise an exception for it.
- An exception raised in Python while a wheel call runs, including one raised by a caller's `on_result` callback, MUST be caught and returned as an error value.

**MUST NOT statements:**

- A consumer MUST NOT need to parse message text to obtain a status, send count or Retry-After.

### Rationale

Consumers decide whether to retry or back off, and how to fix authentication, from the status, the send count and the Retry-After. Without typed fields these values exist only in English message text. `retry_after` is a transport timing hint; REQ-SAG-0007 governs decision values.

The 0.1.0 error-JSON compatibility rule is withdrawn (2026-11-20), Rand Lee: "We are still using python, schema isn't locked yet. I haven't mentioned locking schema yet. I have tools to do this that aren't on this repo. No hand built checks."

Owner ruling (2026-11-20), Rand Lee: nothing raises; throwing exceptions is prohibited on the repo.

Owner ruling (2026-11-20), Rand Lee: "Python exceptions need to be caught (return error) so app doesn't crash."

Owner ruling (2026-11-21), Rand Lee: "My words are what I want, not excess control resulting." "I would never create a requirement requiring subnanosecond rounding for a http call." "Use time structs for time." "always return descriminated unions w/ error info"

### Success Criteria

**Acceptance Criteria:**

- [ ] A busy error read in Rust and in Python, through the returned error value or the result line's `error`, carries the status, the send count and the last Retry-After as a Duration.
- [ ] `decision.http` over `ProviderBusy`, and `decision.backend` over a 401 `llm.api_status`, carry the cause's fields, `category` included, at the top level.
- [ ] A leaf reports its own fields and its own `category`.
- [ ] `decision.cancelled` carries `attempts` equal to its sends.
- [ ] A Retry-After of `1.1` is kept as a Duration of 1.1 seconds; a malformed header gives no `retry_after`.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0002
- REQ-SAG-0007
- REQ-SAG-0017
- REQ-SAG-0601
- REQ-SAG-0602
