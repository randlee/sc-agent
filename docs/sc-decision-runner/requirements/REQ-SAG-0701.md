# Requirement: Batches of decision requests run in parallel

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0701: Batches of decision requests run in parallel

**Status:** Draft  

### Requirement Statement

Definitions: A batch is the decision requests a caller passes in one call: one JSONL file, one stdin stream or one Python call. A worker sends one request at a time.

**MUST statements:**

- `sc-decision-runner` MUST run the requests of one batch in parallel across workers.
- The number of workers MUST be a global configuration setting with a default of 8.
- Results MUST be returned in the order they finish, each tagged with its session ULID and the input position of the request it answers.
- A failed request MUST be returned as that request's error result while the rest of the batch continues.
- Cancelling the batch MUST stop every request that has not finished.

### Rationale

Tasks send tens of thousands of decisions, and one slow or failed request must not hold up or stop the others.

### Success Criteria

**Acceptance Criteria:**

- [ ] A batch of 40 requests against a test transport that answers each after a delay finishes in about one eighth of the serial time with the default of 8 workers.
- [ ] With workers set to 1, the same batch runs one request at a time.
- [ ] A batch in which one request fails returns that error tagged with its session ULID and returns every other result.
- [ ] Cancelling a running batch returns cancelled results for every unfinished request.

### Dependencies

**Related:**

- ADR-SAG-0001
- REQ-SAG-0601
- NFR-SAG-0001