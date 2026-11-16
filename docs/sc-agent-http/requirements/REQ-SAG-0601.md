# Requirement: Busy resend and holdoff

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0601: Busy resend and holdoff

**Status:** Approved  

### Requirement Statement

Definitions: A busy response is a non-quota HTTP 429 or 5xx (including 529). A send is one transmission of a request; all sends of one request belong to one session. The holdoff is the time a busy request waits before it is resent. `Retry-After` is the HTTP response header that gives that time in seconds.

**MUST statements:**

- A request that receives a busy response MUST be resent unchanged.
- A request whose send has a network failure MUST be resent unchanged, counting toward the same maximum number of sends and 90-second budget.
- An invalid response, including a schema mismatch, MUST be resent unchanged using the same exponential holdoff and jitter, send limit, budget and cancellation.
- The maximum number of sends per request MUST be configurable, with a default of 5.
- A valid `Retry-After` MUST set the holdoff, clamped to 0.25 through 30 seconds, with up to 250 milliseconds of jitter (REQ-SAG-0012).
- A `Retry-After` MUST be valid only when it is 1 to 32 ASCII characters of digits with an optional fractional part.
- When `Retry-After` is absent or invalid, the holdoffs MUST be 1, 2, 4 and 8 seconds, doubling after that and capped at 30 seconds, each with up to 250 milliseconds of jitter.
- Attempts MUST start only within a 90-second budget per request; an attempt already in flight keeps its own request timeout.
- A held-off request MUST wait in the queue until its holdoff ends, and the wait MUST end at once when the request is cancelled.
- When sends or budget run out after a busy response, the result MUST be `HttpError::ProviderBusy` with the reason (`sends` or `budget`), the number of attempts and the last `retry_after_seconds`.

**MUST NOT statements:**

- Holdoff MUST NOT use `sleep()`.
- Authentication (401), quota and responses exceeding the HTTP size limit MUST NOT be resent; they MUST be returned with the remote error id and message.

### Rationale

Providers signal overload with busy responses and expect clients to back off. Holding off in the queue keeps workers free and lets cancellation take effect immediately.

### Success Criteria

**Acceptance Criteria:**

- [ ] With the default configuration, a request answered busy every time is sent 5 times and ends `HttpError::ProviderBusy` with reason `sends` and 5 attempts.
- [ ] With the maximum set to 2, the same request is sent 2 times.
- [ ] A busy response with `Retry-After: 3` holds the request off 3 to 3.25 seconds; `Retry-After: 99` holds it off 30 to 30.25 seconds; `Retry-After: 0` holds it off 0.25 to 0.5 seconds; `Retry-After: abc` falls back to 1, 2, 4, 8 seconds.
- [ ] Cancelling a held-off request ends its wait at once and returns a cancelled result.
- [ ] A send that times out is resent unchanged and counts toward the maximum number of sends; no attempt starts after the 90-second budget, and a request whose last attempt timed out ends with the timeout error.
- [ ] A 402 response is returned after one send with the remote error id and message.
- [ ] A workspace search finds no `sleep(` call.

### Dependencies

**Related:**

- REQ-SAG-0602
- REQ-SAG-0104