# Requirement: Busy resend and holdoff

**Status:** Approved  
**Version:** 0.4.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-21  
**Owner:** Rand Lee  

---

## REQ-SAG-0601: Busy resend and holdoff

**Status:** Approved  

### Requirement Statement

Definitions: A busy response is a non-quota HTTP 429, 502 or 529. A send is one transmission of a request; all sends of one request belong to one session. The holdoff is the time a busy request waits before it is resent. `Retry-After` is the HTTP response header that gives that time in seconds. A provider endpoint is one host reached through one `HttpClient`. The not-before time is the earliest moment a send to that endpoint may start.

**MUST statements:**

- A request that receives a busy response MUST be resent unchanged.
- A request whose send has a network failure MUST be resent unchanged, counting toward the same maximum number of sends.
- The maximum number of sends per request MUST be a fixed count of 5.
- When an `HttpClient` receives a busy reply from a host, the `HttpClient` MUST set a not-before time shared by every send through that client to that host.
- Every later send to the provider endpoint MUST wait until the endpoint's not-before time.
- When the provider gives a `Retry-After`, it MUST set the holdoff.
- When the provider gives no `Retry-After`, the holdoffs MUST back off exponentially; this requirement does not fix their values.
- A held-off request MUST wait in the queue until its holdoff ends, and the wait MUST end at once when the request is cancelled.
- When sends run out on busy replies, the error MUST say busy, with the send count.

**MUST NOT statements:**

- Authentication (401) and quota MUST NOT be resent; they MUST be returned with the remote error id and message.

### Rationale

Providers signal overload with busy responses and expect clients to back off. Holding off in the queue keeps workers free and lets cancellation take effect immediately.

A busy provider is busy for every worker; one worker's holdoff that the others ignore only multiplies the busy replies.

Owner ruling (2026-11-20), Rand Lee: "I did ask for exponential backoff / retry when busy (i.e. 1,2,4,8,..). Nothing precise or fancy." "1,2,4,8 is an example of exp backoff not something to hard code."

Owner ruling (2026-11-21), Rand Lee: "Use time structs for time."

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] A request answered busy every time is sent 5 times and ends with a busy error carrying a send count of 5.
- [ ] A busy response with `Retry-After: 3` holds the request off 3 seconds; with no `Retry-After`, each holdoff is longer than the one before.
- [ ] Cancelling a held-off request ends its wait at once and returns a cancelled result.
- [ ] A send that times out is resent unchanged and counts toward the maximum number of sends, and a request whose last attempt timed out ends with the timeout error.
- [ ] A 402 response is returned after one send with the remote error id and message.
- [ ] After one worker receives a busy reply, a send from another worker to the same host through the same `HttpClient` does not start before the not-before time.
- [ ] A send to a different host through the same `HttpClient` is not delayed by that not-before time.

### Dependencies

**Related:**

- REQ-SAG-0602