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

Definitions: the cache flag is a setting that turns the result cache on or off. The request hash is the SHA-256 digest of the full text sc-agent sends to the model. A cache hit is a query, with the cache flag on, whose request hash and requested model match an existing session row. The cached result is the matching session row's result together with that row's session identifier. A session row records an error when its result is an error rather than a value. A batch is one call that sends several requests; each request is an item.

**MUST statements:**

- When no cache setting is given, sc-agent MUST treat the cache flag as on.
- On a cache hit, sc-agent MUST return the cached result.
- When a batch is sent, sc-agent MUST decide a cache hit for each item on its own.

**MUST NOT statements:**

- On a cache hit, sc-agent MUST NOT make a model call.
- On a cache hit, sc-agent MUST NOT write a new session row.
- When the matching session row records an error, sc-agent MUST NOT treat the query as a cache hit.

### Rationale

Tools resend the same request often; answering it from the session log saves the API call. The key includes the model because different models answer differently. A per-item cache means a batch of 10 with 2 changed items makes 2 model calls. The cache also lets offline tests run without a model.

### Success Criteria

**Acceptance Criteria:**

- [ ] A unit test: with no cache setting, two sends of one request to one model make one model call.
- [ ] A unit test: the second of two identical sends returns the first's result and `session_id`.
- [ ] A unit test: when the only matching session row records an error, a resend makes a model call.
- [ ] A unit test: a batch of 10 items, 2 of them changed since a previous run, makes 2 model calls.
- [ ] A unit test: on a cache hit, the model client records no call.
- [ ] A unit test: on a cache hit, the session row count does not change.

### Dependencies

**Related:**

- REQ-SAG-0005
- REQ-SAG-0016
- REQ-SAG-0019
- REQ-SAG-0702
- REQ-SAG-0801
- ADR-SAG-0019
- ADR-SAG-0021
