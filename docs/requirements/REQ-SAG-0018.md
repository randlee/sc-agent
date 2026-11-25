# Requirement: Python consumers integrate through one generated module rendered from a template shipped in the sc-decidr wheel

**Status:** Approved  
**Version:** 0.8.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0018: Python consumers integrate through one generated module rendered from a template shipped in the sc-decidr wheel

**Status:** Approved  

### Requirement Statement

Definitions:

- The consumer template is the sc-compose template `sc_agent/templates/consumer.py.j2`, which ships inside the sc-decidr wheel. It is rendered with sc-compose at or above the sc-compose minimum. sc-compose is an external tool, not a dependency of either wheel.
- The sc-compose minimum is the `minimum_version` of the `sc-compose` entry under `requires.cli` in the consumer template's front matter. It is declared, not enforced.
- The sc-decidr wheel holds both the decidr and the sitr Python API (ADR-SAG-0021), so they always have the same version.
- The consumer module is the Python file a consumer renders from the consumer template.
- The template variables are `client_name`, `sitr` (`true` or `false`, selecting the syntax-query section), and `sc_decidr_version`. Model selectors are passed unchanged as call arguments.
- The client name is the template variable `client_name`. It names the consumer module's Logger and is the `agent_role` the module passes.
- The public API is `send`, `send_batch`, `digest`, `logger`, `log`, `flush` and `Cancel`, as REQ-SAG-0016, REQ-SAG-0017 and REQ-SAG-0702 define them, plus `sc_agent.sitr` for the sitr section.
- The session tags are the `project` and `agent_role` arguments of `send` and `send_batch`. They tag each session-log record with the consumer's repository and the calling agent's role.
- The session log is the wheel's record of every query and response, written to the wheel's default location.
- The logging wiring is the consumer module's code that obtains one Logger with `logger(client name)`, or takes the consumer's own Logger, and passes it to every public-API call.
- The error mapping is the consumer module's code that reads an error value's `category` and maps that category to a consumer code through a category-to-code map the consumer passes in.
- The sitr section is the part of the consumer module that runs tree-sitter extraction; its output is passed into the consumer's prompt authoring.
- Consumer logic is prompt authoring and the processing of results and decision trees. It stays in the consumer.
- The shared consumer code is what the wheel does not support and both consumers share: fixture loading and coverage, batch settings and ids, result-line encoding, attempt counting and the remote-judge consent gate.
- An extension point is a template variable or a named block of the consumer template. Consumer-specific values reach the consumer module only through extension points, so later consumers can use the same template.
- The wheel README is `crates/sc-agent-py/README.md`.
- A decision call is a call to `send` or `send_batch`.
- A covered release is an sc-agent release of version 0.2.0 or later.
- The permitted API is the public API, the Python standard library, and the consumer's own modules named through an extension point (such as the consent settings module).
- `provider` is the keyword argument of `send` and `send_batch` that forces a provider instead of resolving it from the model selector.
- PyPI is the Python Package Index, pypi.org.
- A run is one execution of one consumer command. A run may send many queries: a batch (an array of queries, each giving one result line) or a decision tree (many queries with consumer processing between them).
- `run_id` is the ULID that identifies one run.
- The result line is the JSON object the consumer module writes for one query of a batch, with the schema tag `jev-batch-result/2`.
- The generic result fields are `schema`, `tool`, `tool_version`, `run_id`, `session_id`, `request_id`, `project`, `agent_role`, `threshold`, `verdict`, `min_confidence`, `leaning`, `ok` and `error`.
- The session data is what the session row holds: `request`, `raw_response`, `answers`, `model_answered`, `attempts`, `retry_count` and `sends`.
- A tool record is a record a consumer writes for its own data, such as plan-anchor's `anchor_id` and `candidate_id` or judge-jev's record id.
- A dry run is a consumer command run that composes its requests without sending them.
- The session lookup helper is the consumer template function that reads a session row by its `session_id`.
- An intentional resend is a send of a request identical to one already sent, made on purpose: a repeat, a stability or variance check, or a near-threshold re-ask.

**MUST statements:**

- The consumer template MUST ship in the sc-decidr wheel as package data.
- The consumer module MUST make the decision call through `send` or `send_batch`.
- The consumer module MUST pass the model selector through unchanged.
- Every decision call in the consumer module MUST pass the session tags.
- The consumer module MUST contain the logging wiring.
- The consumer module MUST contain the error mapping.
- Extension points MUST be the only template inputs that change the consumer module's code.
- The consumer template MUST carry the shared consumer code.
- A consumer-specific value MUST reach the consumer module only through an extension point.
- The consumer template MUST be tested against the sc-decidr wheel of the same release.
- PyPI MUST hold the sc-decidr wheel of every covered release.
- The consumer module MUST use `sc_agent.sitr` only inside the sitr section.
- The sitr section MUST stay separate from the decision call.
- The consumer module MUST call only the permitted API.
- Every decision call in the consumer module MUST pass a `Cancel`.
- The consumer module MUST cancel that `Cancel` on `KeyboardInterrupt`.
- The public API MUST stay documented in `sc_agent/__init__.pyi`.
- The public API MUST stay documented in the wheel README.
- The wheel README MUST name the consumer template.
- The wheel README MUST give the consumer template's render command.
- The consumer module MUST mint `run_id` once per run, before the run's first decision call.
- Every result line of a run MUST carry that run's `run_id`.
- The consumer template MUST write every result line with the schema tag `jev-batch-result/2`.
- A result line MUST carry the generic result fields.
- A result line's `session_id` MUST be the `session_id` its send returned, or null when the send returned none.
- Every tool record derived from a query MUST carry the run's `run_id` and that query's `session_id`.
- The consumer template MUST provide the session lookup helper.
- The consumer module MUST find a run's queries through the `session_id`s in that run's result lines and tool records.
- The consumer module MUST pass the caller's `cache` argument (REQ-SAG-0703) to every decision call.
- An intentional resend MUST pass `cache=False`.
- A dry run MUST check that its input and arguments compose into a valid request.
- A dry run MUST show each request it would send on stdout, or in the file named by `--out`.

**MUST NOT statements:**

- The consumer module MUST NOT contain consumer logic.
- The consumer module MUST NOT set `provider`.
- The consumer module MUST NOT parse error message text.
- A result line MUST NOT carry any field other than the generic result fields.
- A result line MUST NOT carry session data.
- A tool record MUST NOT carry session data.
- The general log records of a consumer MUST NOT carry session data.
- A record MUST NOT need a query's `session_id` before that query's send returns.
- A dry run MUST NOT write a session row.
- A dry run MUST NOT write a result line.
- A dry run MUST NOT write a run record or a tool record.

### Rationale

The owner requires judge-jev, plan-anchor and every later Python consumer to integrate identically for errors, API keys, model selection (the jev, haiku, luna, decider and laya alternates), the injectable general logger and session logging. They differ only in prompt authoring and in processing results and decision trees. Session logging is always on and always at the wheel's default location, so every consumer's Jev data takes the same path and can be filtered by repository and agent. A hand-copied recipe already diverged between two consumers (ADR-SAG-0019, Context). One template rendered by every consumer removes that divergence. Shipping the template as package data makes its version the installed wheel's version. Leaving `provider` unset keeps the jev, haiku, luna, decider and laya alternates available.

Owner ruling (2026-11-20), Rand Lee: "Keyring is super basic."

Owner ruling (2026-11-21), Rand Lee: the shared template carries what the wheel does not support and both consumers share (fixture loading and coverage, batch settings and ids, result-line encoding, attempt counting, the remote-judge consent gate), with consumer-specific values only through template variables or named blocks, as extension points for future consumers.

Owner ruling (2026-11-21), Rand Lee: "My words are what I want, not excess control resulting." "No hand built checks." "Keyring is super basic." "always return descriminated unions w/ error info"

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

Owner rulings (2026-11-24), Rand Lee, recorded in `docs/plans/phase-j/audit-trail-log-normalization-SQLite.md`: a tool references `session_id` and never copies the request or the response; the shared result line holds generic fields only and tool ids live in tool records; `run_id` is a ULID minted once per run; a dry run writes nothing as if it were real. A `session_id` is known only after the send returns, and a cache hit returns an older one, so the result line and the tool records hold the link from a run to its queries.

### Success Criteria

**Acceptance Criteria:**

- [ ] A sc-agent test renders the consumer template with the plan-anchor (`sitr=false`) and judge-jev (`sitr=true`) configurations, using `client_name` and the installed `sc_decidr_version` and runs each rendered module against the loopback runner (`crates/sc-agent-py/tests/loopback.py`):
  - a success returns the response;
  - an error returns the consumer code from the passed map for its category;
  - the session log has one record per query and response, carrying the session tags the call was given;
  - the injected Logger receives the call's events.
- [ ] The sc-decidr wheel of the released version is installable from PyPI.
- [ ] An offline test renders with `sitr` true: extraction output from a checked-in source file reaches a dry-run request's prompt.
- [ ] A rendered module with `sitr` false makes no `sc_agent.sitr` call.
- [ ] A `KeyboardInterrupt` during a rendered module's batch cancels the `Cancel` passed to its decision call.
- [ ] Scanning both rendered modules finds only wheel references to the permitted API: `send`, `send_batch`, `digest`, `logger`, `log`, `flush`, `Cancel`, and `sc_agent.sitr`.
- [ ] The README of `crates/sc-agent-py` names the template and its render command.
- [ ] A rendered module's batch against the loopback runner writes one `jev-batch-result/2` line per query, each with exactly the generic result fields, one shared ULID `run_id`, and a `session_id` that the session lookup helper finds in the session log.
- [ ] Two runs of the same command carry different `run_id`s.
- [ ] With the cache on, a second identical run writes result lines whose `session_id`s equal the first run's, under the second run's `run_id`, and the session log gains no row.
- [ ] A rendered module's call with `cache=False` adds a new session row for an identical request.
- [ ] A rendered module's dry run prints the would-send requests, writes no result line and adds no session row.

### Dependencies

**Related:**

- ADR-SAG-0019
- REQ-SAG-0016
- REQ-SAG-0017
- REQ-SAG-0702
- REQ-SAG-0703
- NFR-SAG-0003
