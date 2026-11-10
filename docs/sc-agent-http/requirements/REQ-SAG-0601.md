# Requirement: Busy resend and holdoff

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0601: Busy resend and holdoff

**Status:** Draft  

### Requirement Statement

Definitions: A busy response is an HTTP 429, 502 or 529. A send is one transmission of a request; all sends of one request belong to one session. The holdoff is the time a busy request waits before it is resent. `Retry-After` is the HTTP response header that gives that time in seconds.

**MUST statements:**

- A request that receives a busy response MUST be resent unchanged.
- A request whose send times out MUST be resent unchanged, counting toward the same maximum number of sends.
- The maximum number of sends per request MUST be configurable, with a default of 5.
- A valid `Retry-After` MUST set the holdoff, clamped to 0 through 30 seconds.
- A `Retry-After` MUST be valid only when it is 1 to 32 ASCII characters of digits with an optional fractional part.
- When `Retry-After` is absent or invalid, the holdoffs MUST be 1, 2, 4 and 8 seconds, doubling after that and capped at 30 seconds, each with up to 250 milliseconds of jitter.
- Attempts MUST start only within a 90-second budget per request; an attempt already in flight keeps its own request timeout.
- A held-off request MUST wait in the queue until its holdoff ends, and the wait MUST end at once when the request is cancelled.
- When sends or budget run out, the result MUST be `HttpError::ProviderBusy` with the reason (`sends` or `budget`), the number of attempts and the last `retry_after_seconds`.

**MUST NOT statements:**

- Holdoff MUST NOT use `sleep()`.
- Remote errors other than busy responses, including quota errors, MUST NOT be resent; they MUST be returned with the remote error id and message.

### Rationale

Providers signal overload with busy responses and expect clients to back off. Holding off in the queue keeps workers free and lets cancellation take effect immediately.

### Success Criteria

**Acceptance Criteria:**

- [ ] With the default configuration, a request answered busy every time is sent 5 times and ends `HttpError::ProviderBusy` with reason `sends` and 5 attempts.
- [ ] With the maximum set to 2, the same request is sent 2 times.
- [ ] A busy response with `Retry-After: 3` holds the request off 3 seconds; `Retry-After: 99` holds it off 30 seconds; `Retry-After: abc` falls back to 1, 2, 4, 8 seconds.
- [ ] Cancelling a held-off request ends its wait at once and returns a cancelled result.
- [ ] A send that times out every time is sent 5 times with the default configuration and ends with the timeout error.
- [ ] A 402 response is returned after one send with the remote error id and message.
- [ ] A workspace search finds no `sleep(` call.

### Dependencies

**Related:**

- REQ-SAG-0602
- REQ-SAG-0104