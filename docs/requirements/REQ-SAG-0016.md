# Requirement: Error values carry the send count and category as typed fields

**Status:** Approved  
**Version:** 0.8.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0016: Error values carry the send count and category as typed fields

**Status:** Approved  

### Requirement Statement

Definitions:

- An error value is the `error` object that the CLI, the Python binding, session rows and result lines carry. It has an `id`, a `message` and an `action`.
- The typed fields are two optional fields of an error value:
  - `attempts`, the sends made with the first included;
  - `category`, the error category that REQ-SAG-0017 assigns.

**MUST statements:**

- An error value MUST have the typed fields.
- A typed field that is not set MUST be omitted from serialized JSON.
- A busy error MUST carry the number of sends.
- The sc-decidr wheel MUST return a failed request as an error value carrying the typed fields, and MUST NOT raise an exception for it.
- An exception raised in Python while a wheel call runs, including one raised by a caller's `on_result` callback, MUST be caught and returned as an error value.

**MUST NOT statements:**

- A consumer MUST NOT need to parse message text to obtain a send count or category.

### Rationale

Consumers decide whether to resubmit from the category and the send count. Without typed fields these values exist only in English message text.

The 0.1.0 error-JSON compatibility rule is withdrawn (2026-11-20), Rand Lee: "We are still using python, schema isn't locked yet. I haven't mentioned locking schema yet. I have tools to do this that aren't on this repo. No hand built checks."

Owner ruling (2026-11-20), Rand Lee: nothing raises; throwing exceptions is prohibited on the repo.

Owner ruling (2026-11-20), Rand Lee: "Python exceptions need to be caught (return error) so app doesn't crash."

Owner ruling (2026-11-21), Rand Lee: "My words are what I want, not excess control resulting." "I would never create a requirement requiring subnanosecond rounding for a http call." "Use time structs for time." "always return descriminated unions w/ error info"

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] A busy error read in Rust and in Python, through the returned error value or the result line's `error`, carries the send count.
- [ ] Each error value reports its own `category`.
- [ ] `decision.cancelled` carries `attempts` equal to its sends.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0002
- REQ-SAG-0007
- REQ-SAG-0017
- REQ-SAG-0601
- REQ-SAG-0602
