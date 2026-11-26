# Requirement: Provider-wide busy pause

**Status:** Approved  
**Version:** 0.2.1  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0012: Provider-wide busy pause

**Status:** Approved  

### Requirement Statement

Definitions: A provider endpoint is one host reached through one `HttpClient`. The not-before time is the earliest moment a send to that endpoint may start. A busy reply is a non-quota HTTP 429, 502 or 529. `Retry-After` is the HTTP response header giving the wait in seconds. The fallback wait is an exponentially increasing delay before a retry (REQ-SAG-0601); its values are not fixed.

**MUST statements:**

- When an `HttpClient` receives a busy reply from a host, the `HttpClient` MUST set a not-before time shared by every send through that client to that host.
- Every later send to the provider endpoint MUST wait until the endpoint's not-before time.
- When a busy reply includes `Retry-After`, every sender using that `HttpClient` MUST wait the specified number of seconds.
- When a busy reply has no `Retry-After`, every sender using that `HttpClient` MUST wait for the fallback wait.

### Rationale

A busy provider is busy for every worker; one worker's holdoff that the others ignore only multiplies the busy replies.

Owner ruling (2026-11-21), Rand Lee: "1,2,4,8 is an example of exp backoff not something to hard code."

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