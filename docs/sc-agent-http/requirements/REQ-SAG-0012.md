# Requirement: Provider-wide busy pause

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0012: Provider-wide busy pause

**Status:** Approved  

### Requirement Statement

Definitions: A provider endpoint is one host reached through one `HttpClient`. The not-before time is the earliest moment a send to that endpoint may start.

**MUST statements:**

- A busy reply from a provider endpoint MUST set a not-before time shared by every send through that `HttpClient` to that host.
- Every later send to that endpoint MUST wait until its not-before time.
- A busy reply MUST hold off every sender on that client until its Retry-After, or an exponential backoff when there is none.

### Rationale

A busy provider is busy for every worker; one worker's holdoff that the others ignore only multiplies the busy replies.

Owner ruling (2026-11-21), Rand Lee: "My words are what I want, not excess control resulting." "1,2,4,8 is an example of exp backoff not something to hard code."

### Success Criteria

**Acceptance Criteria:**

- [ ] After one worker receives a busy reply, a send from another worker to the same host through the same `HttpClient` does not start before the not-before time.
- [ ] A send to a different host through the same `HttpClient` is not delayed by that not-before time.

### Dependencies

**Requires:**

- REQ-SAG-0601

**Related:**

- REQ-SAG-0602
- REQ-SAG-0010