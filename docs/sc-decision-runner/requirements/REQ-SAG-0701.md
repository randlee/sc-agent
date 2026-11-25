# Requirement: Batches of decision requests run in parallel

**Status:** Approved  
**Version:** 0.4.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0701: Batches of decision requests run in parallel

**Status:** Approved  

### Requirement Statement

Definitions: A batch is the decision requests a caller passes in one call: one JSONL file, one stdin stream or one Python call. A worker sends one request at a time.

**MUST statements:**

- `sc-agent`'s `send_batch` MUST run the requests of one batch in parallel across workers.
- The number of workers MUST be a configuration setting with a default of 8.
- Results may be returned in any order. Each result MUST carry its session ULID and the input position of the request it answers.
- On a cache hit (REQ-SAG-0703), a result MUST carry the `session_id` of the existing session row that holds its result.
- A failed request MUST be returned as that request's error result while the rest of the batch continues.
- Cancelling the batch MUST stop every request that has not finished.

### Rationale

Tasks send tens of thousands of decisions, and one slow or failed request must not hold up or stop the others.

Owner ruling (2026-11-20), Rand Lee: "I do not require results in request order."

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

### Success Criteria

**Acceptance Criteria:**

- [ ] With the default of 8 workers, a batch of 40 requests sent to a test server that holds every reply until 8 requests are in flight completes, and the server never sees more than 8 requests in flight.
- [ ] With workers set to 1, the test server never sees more than one request in flight from the same batch.
- [ ] A batch in which one request fails returns that error tagged with its session ULID and returns every other result.
- [ ] Cancelling a running batch returns cancelled results for every unfinished request.
- [ ] A batch request that is a cache hit returns the `session_id` of the row that already holds its result, and no new row is written.

### Dependencies

**Related:**

- ADR-SAG-0001
- REQ-SAG-0601
- NFR-SAG-0001
- REQ-SAG-0703