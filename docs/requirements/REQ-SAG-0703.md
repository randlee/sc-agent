# Requirement: Result cache: a repeated request returns the existing session's result without a model call

**Status:** Approved  
**Version:** 0.4.1  
**Created:** 2026-11-24  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0703: Result cache: a repeated request returns the existing session's result without a model call

**Status:** Approved  

### Requirement Statement

Definitions: A query is one decision request sent through `send`, or one request of a `send_batch` call. The request text is the exact request body sc-agent sends to the decision model for one query, in the wire format of the query's provider. The request hash is the SHA-256 digest of the request text, computed by sc-sha. The requested model is the query's `model_requested` (REQ-SAG-0005), taken from its resolved model (REQ-SAG-0102). A Run is the run context object (ADR-SAG-0019 decision 14). A query's session log is the `sessions.jsonl` of the query's repo, agent role and project (REQ-SAG-0005). An error row is a session row that records an error instead of a decision. A matching row is a row in the query's session log with the same request hash and the same requested model that is not an error row. The most recent matching row is the matching row with the latest `session_id` ULID. A link record and the run log are defined in REQ-SAG-0005. The cache flag says whether the result cache is on for a query. A cache hit is a query whose cache flag is on and that has a matching row. A job is one request of a `send_batch` call. The cache policy is the cache flag a Run is created with.

**MUST statements:**

- The cache flag MUST be a configuration key, `cache`, with the default `true`.
- The `cache` key MUST be accepted wherever REQ-SAG-0019 accepts `workers`.
- `sc_agent.Run` MUST accept an optional cache policy, `cache`, that overrides the configured flag for that Run.
- `run.send` and `run.send_batch` MUST accept an optional per-call `cache` argument.
- A call's `cache` MUST override the Run's cache policy for that call only, and for every job of that call.
- The cache key MUST be the request hash together with the requested model, which is the resolved model's fixed id; a query's model MUST stay in its request text, its session row and its cache lookup.
- Before each model call whose cache flag is on, sc-agent MUST look for a matching row in the query's session log.
- When the query's session log cannot be read, the lookup MUST return the error value `session.unreadable` (REQ-SAG-0022) and the query MUST NOT be sent.
- A cache hit's result MUST carry `attempts: 0`.
- On a cache hit, sc-agent MUST return the most recent matching row's result with `cached: true`.
- On a cache hit, the returned `session_id` MUST be the most recent matching row's `session_id`.
- On a cache hit, sc-agent MUST append a link record with the Run's `run_id`, the returned `session_id` and `cached: true` to the run log (REQ-SAG-0005).
- A result that is not a cache hit MUST carry `cached: false`.
- In a batch, sc-agent MUST decide a cache hit for each job on its own.
- When a query's cache flag is off, sc-agent MUST send the query and write its session row.

**MUST NOT statements:**

- On a cache hit, sc-agent MUST NOT make a model call.
- On a cache hit, sc-agent MUST NOT write a session row.
- The cache key MUST NOT include `model_answered`.
- The cache MUST NOT use a row from another repo, agent role or project.
- The cache MUST NOT use the question digest of REQ-SAG-0202 as its key.
- An error row MUST NOT be a cache hit.
- `run.send_batch` MUST NOT accept a cache flag per job.

### Rationale

Tools resend the same request often, for example when only items 2 and 5 of a 10-item batch changed; with the cache, that batch makes 2 API calls. `model_answered` is not known before the call, so the key uses the requested model. jev, haiku and decider answer differently, so the model is part of the key. A caller that resends an identical request on purpose (repeats, stability or variance checks, near-threshold re-asks) passes `cache=False` and gets a new session. A caller that needs both policies sends two batches; each job of a batch is still looked up on its own. A cache hit returns an older `session_id`, so the session row cannot name the runs it serves; the link record written on every hit, and the run's own result and tool records (REQ-SAG-0018), hold that link. An error is not an answer worth reusing, so an error row is never a hit and the next send retries the model.

Owner rulings (2026-11-24), Rand Lee, recorded in `docs/plans/phase-j/audit-trail-log-normalization-SQLite.md`: result cache, a flag, default on; before each model call, if the sha256 of the full text sent to Jev and the model already have a session row, that row's result is returned and no API call is made; it applies per item for batches and streamed input. Round 3: a cache hit still writes a link record with `cached: true`, but no session row.

Owner ruling 2026-11-24, Rand Lee: "cache keys require model."

Owner ruling (2026-11-24), recorded in the same design doc: a cache hit returns the most recent successful matching row; error rows are never hits.

Owner ruling 2026-11-24 (plan review, ADR-SAG-0003): no per-job cache list; one cache policy per call, on the Run or overridden per call; a caller needing both sends two batches; caching still checks each item individually.

### Success Criteria

**Acceptance Criteria:**

- [ ] Two sends of the same request with the same requested model, cache on, against a loopback runner that counts calls: one model call; the second result carries `cached: true` and the first result's `session_id`; the session log holds one row.
- [ ] The same request sent with two different requested models makes two model calls and writes two rows.
- [ ] A batch of 10 jobs in which 8 have matching rows makes exactly 2 model calls and writes 2 rows; the 8 hits carry their matching rows' `session_id`s.
- [ ] A send with `cache=False` of a request that has a matching row makes a model call and writes a row with a new `session_id`.
- [ ] With every job of two batches pre-logged, a batch with `cache=True` makes no model call and a batch of one job with `cache=False` makes exactly one.
- [ ] A Run created with `cache=False` sends a request that has a matching row; a call through it with `cache=True` returns the hit.
- [ ] With the configuration key `cache` set to `false`, a request that has a matching row makes a model call.
- [ ] A matching row in another project's session log is not a cache hit.
- [ ] The same request sent to `decider-2b-v11` and to `decider-4b-v2.1` makes two model calls and writes two rows, each keyed on its row's fixed id.
- [ ] With the query's session log unreadable, a send with cache on returns `session.unreadable` and makes no model call.
- [ ] A cache hit's result carries `attempts: 0`.
- [ ] When the only row with the same request hash and requested model is an error row, a send with cache on makes a model call and writes a new row.
- [ ] With two successful matching rows, a cache hit returns the result and `session_id` of the one with the later `session_id`.
- [ ] A cache hit through a Run with `run_id` R adds one link record `{run_id: R, session_id: <matching row's>, cached: true}` to the run log and no session row.

### Dependencies

**Related:**

- REQ-SAG-0005
- REQ-SAG-0018
- REQ-SAG-0019
- REQ-SAG-0202
- REQ-SAG-0701
- REQ-SAG-0702
- ADR-SAG-0021
