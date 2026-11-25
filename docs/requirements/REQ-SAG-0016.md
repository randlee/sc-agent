# Requirement: Error values carry the send count and category as typed fields

**Status:** Approved  
**Version:** 0.9.1  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0016: Error values carry the send count and category as typed fields

**Status:** Approved  

### Requirement Statement

Definitions:

- The `sc-decidr` Python wheel contains the decidr and sitr Python APIs (REQ-SAG-0018).
- A busy error has the category `busy` (REQ-SAG-0017).
- A Python exception raised while a wheel call runs includes an exception raised by a caller's `on_result` callback.

- An error value is the `error` object that the CLI, the Python binding, session rows and result lines carry. It has at least an `id`, a `message` and an `action`, and may carry more fields.
- The typed fields of an error value are:
  - `category`, the error category that REQ-SAG-0017 assigns, always set;
  - `attempts`, the sends made with the first included, set when the error follows a send.

**MUST statements:**

- An error value MUST have the typed fields that apply to it.
- A typed field that is not set MUST be omitted from serialized JSON.
- Every busy error MUST carry its number of sends in `attempts`.
- If a request fails, the `sc-decidr` Python wheel MUST return an error value carrying the typed fields.
- An exception raised in Python while a wheel call runs MUST be caught.
- An exception raised in Python while a wheel call runs MUST be returned as an error value.

**MUST NOT statements:**

- For a failed request, the `sc-decidr` Python wheel MUST NOT raise an exception.
- A consumer MUST NOT need to parse message text to obtain a send count.
- A consumer MUST NOT need to parse message text to obtain an error category.

### Rationale

Consumers decide whether to resubmit from the category and the send count. Without typed fields these values exist only in English message text.

The 0.1.0 error-JSON compatibility rule is withdrawn (2026-11-20), Rand Lee: "We are still using python, schema isn't locked yet. I haven't mentioned locking schema yet. I have tools to do this that aren't on this repo. No hand built checks."

Owner ruling (2026-11-20), Rand Lee: nothing raises; throwing exceptions is prohibited on the repo.

Owner ruling (2026-11-20), Rand Lee: "Python exceptions need to be caught (return error) so app doesn't crash."

Owner ruling (2026-11-21), Rand Lee: "I would never create a requirement requiring subnanosecond rounding for a http call." "Use time structs for time." "always return descriminated unions w/ error info"

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

Owner ruling (2026-11-21), Rand Lee: "Extra fields are fine, make sure adr doesn't limit too much. I don't want findings for returning more."

### Success Criteria

**Acceptance Criteria:**

- [ ] A unit test verifies that a busy error read in Rust and in Python, through the returned error value or the result line's `error`, carries the send count.
- [ ] A unit test verifies that each error value reports its own `category`.
- [ ] A unit test verifies that `decision.cancelled` carries `attempts` equal to its sends.
- [ ] A unit test verifies that Python exceptions raised during a wheel call, including in `on_result`, are caught and returned as error values.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0002
- REQ-SAG-0007
- REQ-SAG-0017
- REQ-SAG-0601
- REQ-SAG-0602
