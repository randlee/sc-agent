# Requirement: Result cache: a repeated request returns the existing session's result without a model call

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-24  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0703: Result cache: a repeated request returns the existing session's result without a model call

**Status:** Approved  

### Requirement Statement

Definitions: A query is one decision request sent through `send`, or one request of a `send_batch` call. The request text is the full text sc-agent sends to the decision model for one query. The request hash is the SHA-256 digest of the request text, computed by sc-sha. The requested model is the resolved requested model id, `model_requested` (REQ-SAG-0005). A query's session log is the `sessions.jsonl` of the query's repo, agent role and project (REQ-SAG-0005). A matching row is a row in the query's session log with the same request hash and the same requested model. The cache flag says whether the result cache is on for a query. A cache hit is a query whose cache flag is on and that has a matching row. A job is one request of a `send_batch` call.

**MUST statements:**

- The cache flag MUST be a configuration key, `cache`, with the default `true`.
- The `cache` key MUST be accepted wherever REQ-SAG-0019 accepts `workers`.
- `sc_agent.send` MUST accept an optional per-call `cache` argument.
- `sc_agent.send_batch` MUST accept an optional per-call `cache` argument.
- `sc_agent.send_batch` MUST accept an optional `cache` on each job.
- A job's `cache` MUST override the call's `cache` for that job.
- A call's `cache` MUST override the configured flag for that call only.
- Before each model call whose cache flag is on, sc-agent MUST look for a matching row in the query's session log.
- On a cache hit, sc-agent MUST return the matching row's result with `cached: true`.
- On a cache hit, the returned `session_id` MUST be the matching row's `session_id`.
- A result that is not a cache hit MUST carry `cached: false`.
- In a batch, sc-agent MUST decide a cache hit for each job on its own.
- When a query's cache flag is off, sc-agent MUST send the query and write its session row.

**MUST NOT statements:**

- On a cache hit, sc-agent MUST NOT make a model call.
- On a cache hit, sc-agent MUST NOT write a session row.
- The cache key MUST NOT include `model_answered`.
- The cache MUST NOT use a row from another repo, agent role or project.
- The cache MUST NOT use the question digest of REQ-SAG-0202 as its key.

### Rationale

Tools resend the same request often, for example when only items 2 and 5 of a 10-item batch changed; with the cache, that batch makes 2 API calls. `model_answered` is not known before the call, so the key uses the requested model. jev, haiku and decider answer differently, so the model is part of the key. A caller that resends an identical request on purpose (repeats, stability or variance checks, near-threshold re-asks) passes `cache=False` and gets a new session. A cache hit returns an older `session_id`, so a run finds its queries through its own result and tool records (REQ-SAG-0018).

Owner rulings (2026-11-24), Rand Lee, recorded in `docs/plans/phase-j/audit-trail-log-normalization-SQLite.md`: result cache, a flag, default on; before each model call, if the sha256 of the full text sent to Jev and the model already have a session row, that row's result is returned and no API call is made; it applies per item for batches and streamed input.

### Success Criteria

**Acceptance Criteria:**

- [ ] Two sends of the same request with the same requested model, cache on, against a loopback runner that counts calls: one model call; the second result carries `cached: true` and the first result's `session_id`; the session log holds one row.
- [ ] The same request sent with two different requested models makes two model calls and writes two rows.
- [ ] A batch of 10 jobs in which 8 have matching rows makes exactly 2 model calls and writes 2 rows; the 8 hits carry their matching rows' `session_id`s.
- [ ] A send with `cache=False` of a request that has a matching row makes a model call and writes a row with a new `session_id`.
- [ ] A batch with `cache=True` and one job with `cache=False` sends only that job when every job has a matching row.
- [ ] With the configuration key `cache` set to `false`, a request that has a matching row makes a model call.
- [ ] A matching row in another project's session log is not a cache hit.

### Dependencies

**Related:**

- REQ-SAG-0005
- REQ-SAG-0018
- REQ-SAG-0019
- REQ-SAG-0202
- REQ-SAG-0701
- REQ-SAG-0702
- ADR-SAG-0021
