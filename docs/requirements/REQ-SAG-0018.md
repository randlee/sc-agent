# Requirement: Python consumers integrate through one generated module rendered from a template shipped in the sc-decidr wheel

**Status:** Approved  
**Version:** 1.6.1  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0018: Python consumers integrate through one generated module rendered from a template shipped in the sc-decidr wheel

**Status:** Approved  

### Requirement Statement

Definitions (template contents: ADR-SAG-0019 decision 1):

- The consumer template is `sc_agent/templates/consumer.py.j2`; a consumer renders its consumer module from it; a tool is such a consumer.
- The Run is `sc_agent.Run` (ADR-SAG-0019 decision 14) and its model value is `run.model` (decision 13); a decision call is its `send` or `send_batch`. A run is one execution of one consumer command; `run_id` is its ULID.
- The session tags are the Run's `project` and each call's `agent_role`. The session log holds every query and response; the session-row reader is `sc_agent.session_rows` (REQ-SAG-0022).
- The logging wiring gives one Logger (`logger(client name)` or the consumer's) to every Run and sitr call. The error mapping maps an error's `category` to a consumer code through the consumer's map; the generic error code is its code for `other`.
- The consent gate checks remote-judge consent before a send. The sitr section runs tree-sitter extraction. Consumer logic is prompt authoring and result handling.
- The shared consumer code is fixture loading and coverage, batch settings and ids, result-line encoding and the consent gate. Extension points are template variables and named blocks.
- The public API is `Run`, `Model`, `TYPESAFE`, `TestSeams`, `session_rows`, `provider_keys`, `digest`, `logger`, `log`, `flush` and `sc_agent.sitr`; the permitted API adds the standard library and the consumer's modules named through extension points. The wheel README is `crates/sc-agent-py/README.md`. Covered releases are 0.2.0 and later. A copy: NFR-SAG-0004.
- The result line is the JSON object per batch query; its generic result fields are `schema`, `tool`, `tool_version`, `run_id`, `session_id`, `request_id`, `project`, `agent_role`, `threshold`, `verdict`, `min_confidence`, `leaning`, `ok` and `error`. Session data is `request`, `raw_response`, `answers`, `model_answered`, `attempts`, `retry_count` and `sends`. A tool record is a consumer's own record.
- A dry run composes requests without sending; a fixture run answers from a fixture file; an intentional resend repeats a request on purpose. Run log and link records: REQ-SAG-0005.
- A provider key is a name `sc_agent.provider_keys()` returns. The secret redaction replaces each provider key's environment value and every `Bearer <token>` with `[redacted]` before text is logged, written or shown; `.env` and keyring values stay in Rust, which redacts its own error text.
- The session seeder is `sc_agent.testing.seed_session`.

**MUST statements:**

- The consumer template MUST ship in the sc-decidr wheel as package data.
- The consumer module MUST make every decision call through a Run.
- The consumer module MUST pass its caller's model selector unchanged to the Run.
- Every decision call in the consumer module MUST pass the session tags.
- The consumer module MUST contain the logging wiring.
- The consumer module MUST contain the error mapping.
- The category-to-code map MUST have an entry for each category of REQ-SAG-0017.
- The error mapping MUST map a category that is not in REQ-SAG-0017 to the generic error code.
- The sc-decidr wheel MUST ship the session seeder.
- Extension points MUST be the only template inputs that change the consumer module's code.
- The consumer template MUST carry the shared consumer code.
- A consumer-specific value MUST reach the consumer module only through an extension point.
- The consumer template MUST be tested against the sc-decidr wheel of the same release.
- PyPI MUST hold the sc-decidr wheel of every covered release.
- The consumer module MUST use `sc_agent.sitr` only inside the sitr section.
- The consumer module MUST call only the permitted API.
- The consumer module MUST call `cancel()` on the running Run on `KeyboardInterrupt`.
- The public API MUST stay documented in `sc_agent/__init__.pyi`.
- The public API MUST stay documented in the wheel README.
- The wheel README MUST name the consumer template.
- The wheel README MUST give the consumer template's render command.
- The consumer module MUST create one Run per run before the run's first decision call and close it after its last.
- Every result line of a run MUST carry that Run's `run_id`, or null when the Run failed creation.
- The consumer template MUST write every result line with the schema tag `jev-batch-result/2`.
- A result line MUST carry the generic result fields.
- A result line's `session_id` MUST be the `session_id` its send returned, or null when the send returned none.
- A result line's `project` MUST be the project/phase its Run resolved, or null when the Run was not created.
- Every tool record derived from a query MUST carry the run's `run_id` and that query's `session_id`.
- The consumer module MUST read session rows only through the session-row reader.
- The consumer module MUST find a run's queries through the run log's link records for that `run_id`, or through the `session_id`s in that run's result lines and tool records.
- The consumer module MUST pass the caller's cache policy (REQ-SAG-0703) to its Run.
- The consumer module MUST pass a caller's per-call `cache` override to the decision call it applies to.
- An intentional resend MUST pass `cache=False`.
- A dry run MUST check that its input and arguments compose into a valid request.
- A dry run MUST show each request it would send on stdout, or in the file named by `--out`.
- Every tool MUST complete a decision call with `--model haiku` when `TYPESAFE_API_KEY` is absent and a provider key of a provider serving `haiku` is present.
- Every tool MUST complete a decision call with `--model luna` when `TYPESAFE_API_KEY` is absent and a provider key of a provider serving `luna` is present.
- The secret redaction MUST cover the value of every provider key.
- The consent gate MUST ask for consent only when the model value's provider is the wheel's TypeSafe constant (`sc_agent.TYPESAFE`); a refused consent MUST send nothing.

**MUST NOT statements:**

- The consumer module MUST NOT name a model in code; the caller chooses the model.
- The consumer module MUST NOT contain consumer logic.
- The consumer module MUST NOT choose a provider.
- The consumer template MUST NOT contain a copy.
- The consumer module MUST NOT mint a `run_id` or any other ULID.
- The consumer module MUST NOT compute a session-log path or parse a session-log file.
- The consumer module MUST NOT parse error message text.
- A result line MUST NOT carry any field other than the generic result fields.
- A result line MUST NOT carry session data.
- A tool record MUST NOT carry session data.
- The general log records of a consumer MUST NOT carry session data.
- A record MUST NOT need a query's `session_id` before that query's send returns.
- A dry run MUST NOT write a session row.
- A dry run MUST NOT write a result line.
- A dry run MUST NOT write a run record or a tool record.
- A dry run MUST NOT write a link record.
- A dry run MUST NOT create a Run.
- A fixture run MUST NOT create a Run.
- A fixture run MUST NOT write a result line, a session row or a link record.
- A tool MUST NOT require `TYPESAFE_API_KEY` for a `haiku` or `luna` decision call.
- A test of this requirement MUST NOT use a real provider key, read the platform keyring or set `HOME`.

### Rationale

One shipped template replaces diverged hand-copied integration (ADR-SAG-0019, Context).

Owner rulings (Rand Lee, 2026-11-20 to 2026-11-24): keyring, shared template, audit trail (`docs/plans/phase-j/audit-trail-log-normalization-SQLite.md`; follow-ups: randlee/sc-agent#428), one owner (ADR-SAG-0003), TypeSafe-only consent; lead rulings 2026-11-24.

### Success Criteria

**Acceptance Criteria:** (both consumers' renders, on `crates/sc-agent-py/tests/loopback.py`)

- [ ] A success returns the response; each of the ten REQ-SAG-0017 categories returns its mapped code, an unknown one the generic error code; the session log gets one record per query with its session tags; the injected Logger gets the events.
- [ ] The released wheel installs from PyPI; its README names the template and its render command.
- [ ] With `sitr` true, extraction from a checked-in file reaches a dry-run prompt offline; with `sitr` false no `sc_agent.sitr` call is made.
- [ ] A `KeyboardInterrupt` in a batch cancels its Run and returns cancelled results.
- [ ] A scan finds only permitted-API wheel references, no provider-key name, no ULID minting and no session-log path literal; `provider_keys()` equals the Rust provider table's credential names.
- [ ] A batch of 3 writes 3 `jev-batch-result/2` lines with exactly the generic result fields, the Run's `run_id` and project, and `session_id`s that `session_rows` finds and 3 new link records carry; two runs differ in `run_id`; a failed Run gives null.
- [ ] With the cache on, a repeated run reuses the first run's `session_id`s and adds no session row; `cache=False` adds one.
- [ ] A dry run prints its requests to stdout and to an `--out` file; it and a fixture run create no Run and add no result line, tool record, session row, link record or general-log session data.
- [ ] With `TYPESAFE_API_KEY` unset, `--model haiku` with a dummy `ANTHROPIC_API_KEY` and `--model luna` with a dummy `OPENAI_API_KEY` complete.
- [ ] With every provider key and a `Bearer <token>` as dummy values, the general log, result lines, dry-run output and stdout show `[redacted]`.
- [ ] A TypeSafe model value asks consent, a refusal sends nothing; a non-TypeSafe `haiku` call sends unasked.
- [ ] After `seed_session` writes a row, a real Run with the same options and the cache on returns that answer with `cached: true` and sends nothing.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0016
- REQ-SAG-0017
- REQ-SAG-0702
- REQ-SAG-0703
- NFR-SAG-0003
- NFR-SAG-0004
- REQ-SAG-0022
- REQ-SAG-0005
