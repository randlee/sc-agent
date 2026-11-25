# Requirement: Result cache: a repeated request returns the existing session's result without a model call

**Status:** Approved  
**Version:** 0.6.0  
**Created:** 2026-11-24  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0703: Result cache: a repeated request returns the existing session's result without a model call

**Status:** Approved  

### Requirement Statement

Definitions: sc-agent is the Rust core, with its Python wheel, that sends decision requests and writes the session log. A query, a session row and a query's session log are defined in REQ-SAG-0005. The request hash is the SHA-256 digest, computed by sc-sha, of the exact bytes of the request body sc-agent sends to the provider (in the provider's wire format). The requested model is the query's `model_requested` (REQ-SAG-0005). A successful row is a session row that records no error value (REQ-SAG-0016). The cache is on for a query when its cache flag is true; the cache flag is the configuration key `cache` (REQ-SAG-0019), as overridden for a Run or for one call (ADR-SAG-0019 decision 14). The most recent of several session rows is the one with the latest `session_id`. A cache hit is a query for which the cache is on and the query's session log has a successful row with the query's request hash and requested model.

**MUST statements:**

- When the cache is on and the query's session log has a successful row with the same request hash and requested model, sc-agent MUST return the most recent such row's result.

**MUST NOT statements:**

- On a cache hit, sc-agent MUST NOT make a model call.

### Rationale

Tools resend the same request often; answering it from the session log saves the API call. Owner ruling 2026-11-24, Rand Lee: the key is "the sha256 of the full text sent to Jev and the model", because different models answer differently. An error is not worth reusing, so an error row is never a hit. A send that returned `decision.model_mismatch` (REQ-SAG-0801) wrote a row recording that error (REQ-SAG-0016), so the same request sent again calls the model. A cache hit writes no session row and appends a link record with `cached: true` to the run log (REQ-SAG-0005).

### Success Criteria

**Acceptance Criteria:**

- [ ] Two sends of one request to one model make one model call, and the second returns the first's result and `session_id`; planned in agt-j-2 deliverable J2-7.
- [ ] A different requested model, project or agent role, configuration `cache: false`, or per-call `cache=False` each makes a model call; planned in agt-j-2 deliverable J2-7 (`cache_case` table).
- [ ] With two successful rows for one request hash and requested model, a cache hit returns the result of the row with the later `session_id`, and an error row as the only row with that request hash and requested model makes a model call; planned in agt-j-2 deliverable J2-7.
- [ ] After a send that returned `decision.model_mismatch`, the same request sent again with the cache on makes a model call; planned in agt-j-2 deliverable J2-9.

### Dependencies

**Related:**

- REQ-SAG-0005
- REQ-SAG-0016
- REQ-SAG-0019
- REQ-SAG-0022
- REQ-SAG-0702
- REQ-SAG-0801
- ADR-SAG-0019
- ADR-SAG-0021
