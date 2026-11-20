# Requirement: Busy resend and holdoff

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-20  
**Owner:** Rand Lee  

---

## REQ-SAG-0601: Busy resend and holdoff

**Status:** Approved  

### Requirement Statement

Definitions: A busy response is a non-quota HTTP 429 or 5xx (including 529). A send is one transmission of a request; all sends of one request belong to one session. The holdoff is the time a busy request waits before it is resent. `Retry-After` is the HTTP response header that gives that time in seconds.

**MUST statements:**

- A request that receives a busy response MUST be resent unchanged.
- A request whose send has a network failure MUST be resent unchanged, counting toward the same maximum number of sends.
- An invalid response, including a schema mismatch, MUST be resent unchanged with the same holdoff, send limit and cancellation.
- The maximum number of sends per request MUST be configurable, with a default of 5.
- When the provider gives a `Retry-After`, it MUST set the holdoff.
- When the provider gives no `Retry-After`, the holdoffs MUST back off exponentially; this requirement does not fix their values.
- A held-off request MUST wait in the queue until its holdoff ends, and the wait MUST end at once when the request is cancelled.
- When sends run out after a busy response, the result MUST be `HttpError::ProviderBusy` with the number of attempts and the last `retry_after_seconds`.

**MUST NOT statements:**

- Holdoff MUST NOT use `sleep()`.
- Authentication (401), quota and responses exceeding the HTTP size limit MUST NOT be resent; they MUST be returned with the remote error id and message.

### Rationale

Providers signal overload with busy responses and expect clients to back off. Holding off in the queue keeps workers free and lets cancellation take effect immediately.

Owner ruling (2026-11-20), Rand Lee: "I did ask for exponential backoff / retry when busy (i.e. 1,2,4,8,..). Nothing precise or fancy." "My words are what I want, not excess control resulting." "1,2,4,8 is an example of exp backoff not something to hard code."

### Success Criteria

**Acceptance Criteria:**

- [ ] With the default configuration, a request answered busy every time is sent 5 times and ends `HttpError::ProviderBusy` with 5 attempts.
- [ ] With the maximum set to 2, the same request is sent 2 times.
- [ ] A busy response with `Retry-After: 3` holds the request off 3 seconds; with no `Retry-After`, each holdoff is longer than the one before.
- [ ] Cancelling a held-off request ends its wait at once and returns a cancelled result.
- [ ] A send that times out is resent unchanged and counts toward the maximum number of sends, and a request whose last attempt timed out ends with the timeout error.
- [ ] A 402 response is returned after one send with the remote error id and message.
- [ ] A workspace search finds no `sleep(` call.

### Dependencies

**Related:**

- REQ-SAG-0602
- REQ-SAG-0104