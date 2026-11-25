# Requirement: Python consumers integrate through one generated module rendered from a template shipped in the sc-decidr wheel

**Status:** Approved  
**Version:** 1.3.0  
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
- The template variables are `client_name`, `tool_version` (the consumer's released version), `sitr` (`true` or `false`, selecting the syntax-query section), and `sc_decidr_version`. Model selectors are passed unchanged as call arguments.
- The client name is the template variable `client_name`. It names the consumer module's Logger and is the `agent_role` the module passes.
- The Run is the wheel's `sc_agent.Run` context object (ADR-SAG-0019 decision 14). It is created with the tool identity, the cache policy and the values that do not change between calls; creating it mints the run's ULID `run_id` in Rust, readable as `run.run_id`; it has `send`, `send_batch` and `cancel`, and closing it flushes its Logger.
- The session-row reader is the wheel's `sc_agent.session_rows`, which returns session rows by `session_id` (REQ-SAG-0022).
- The provider-key export is the wheel's `sc_agent.provider_keys()`, which returns the credential name of every provider in the Rust provider table.
- The public API is `Run`, `Model`, `TYPESAFE`, `TestSeams` (for tests), `session_rows`, `provider_keys`, `digest`, `logger`, `log` and `flush`, as REQ-SAG-0016, REQ-SAG-0017 and REQ-SAG-0702 define them, plus `sc_agent.sitr` for the sitr section.
- The session tags are the Run's `project` and each decision call's `agent_role`. They tag each session-log record with the consumer's repository and the calling agent's role.
- The session log is the wheel's record of every query and response, written to the wheel's default location.
- The logging wiring is the consumer module's code that obtains one Logger with `logger(client name)`, or takes the consumer's own Logger, and gives it to every Run and every sitr call.
- The error mapping is the consumer module's code that reads an error value's `category` and maps that category to a consumer code through a category-to-code map the consumer passes in.
- The sitr section is the part of the consumer module that runs tree-sitter extraction; its output is passed into the consumer's prompt authoring.
- Consumer logic is prompt authoring and the processing of results and decision trees. It stays in the consumer.
- The shared consumer code is what the wheel does not support and both consumers share: fixture loading and coverage, batch settings and ids, result-line encoding and the remote-judge consent gate.
- An extension point is a template variable or a named block of the consumer template. Consumer-specific values reach the consumer module only through extension points, so later consumers can use the same template.
- The wheel README is `crates/sc-agent-py/README.md`.
- A decision call is a call to `send` or `send_batch` of a Run.
- A covered release is an sc-agent release of version 0.2.0 or later.
- The permitted API is the public API, the Python standard library, and the consumer's own modules named through an extension point (such as the consent settings module).
- A model value is the typed result of resolving a model selector once (ADR-SAG-0019 decision 13): the Run's model (`run.model`), one per Run. It carries the alias, the model id and the provider.
- PyPI is the Python Package Index, pypi.org.
- A run is one execution of one consumer command. A run may send many queries: a batch (an array of queries, each giving one result line) or a decision tree (many queries with consumer processing between them).
- `run_id` is the ULID that identifies one run.
- The result line is the JSON object the consumer module writes for one query of a batch, with the schema tag `jev-batch-result/2`.
- The generic result fields are `schema`, `tool`, `tool_version`, `run_id`, `session_id`, `request_id`, `project`, `agent_role`, `threshold`, `verdict`, `min_confidence`, `leaning`, `ok` and `error`.
- The session data is what the session row holds: `request`, `raw_response`, `answers`, `model_answered`, `attempts`, `retry_count` and `sends`.
- A tool record is a record a consumer writes for its own data, such as plan-anchor's `anchor_id` and `candidate_id` or judge-jev's record id.
- A dry run is a consumer command run that composes its requests without sending them.
- An intentional resend is a send of a request identical to one already sent, made on purpose: a repeat, a stability or variance check, or a near-threshold re-ask.
- The run log and its link records are defined in REQ-SAG-0005.
- A provider key is the credential name of a provider in the Rust provider table, as the provider-key export returns it.
- The secret redaction is the consumer template's code that replaces the environment value of each provider key, and every `Bearer <token>`, with the placeholder `[redacted]` before text is logged, written or shown. Provider keys read from the `.env` file or the keyring stay in Rust, which redacts them from its own error text.
- A copy is defined in NFR-SAG-0004.
- A tool is a consumer whose module is rendered from the consumer template.
- A fixture run is a run whose answers come from a fixture file given on the command line instead of from sends.
- The consent gate is the template's check of the consumer's remote-judge consent before a send.
- The loopback provider stub is a test HTTP server on the loopback interface that answers as a provider.

**MUST statements:**

- The consumer template MUST ship in the sc-decidr wheel as package data.
- The consumer module MUST make every decision call through a Run.
- The consumer module MUST pass the model selector it receives from its caller through unchanged to the Run.
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
- The consumer module MUST call `cancel()` on the running Run on `KeyboardInterrupt`.
- The public API MUST stay documented in `sc_agent/__init__.pyi`.
- The public API MUST stay documented in the wheel README.
- The wheel README MUST name the consumer template.
- The wheel README MUST give the consumer template's render command.
- The consumer module MUST create one Run per run, before the run's first decision call, and close it after the run's last decision call.
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
- Every tool MUST complete a decision call with `--model haiku` when `TYPESAFE_API_KEY` is absent and the provider key of a provider that serves `haiku` is present.
- Every tool MUST complete a decision call with `--model luna` when `TYPESAFE_API_KEY` is absent and the provider key of a provider that serves `luna` is present.
- The secret redaction MUST cover the value of every provider key.
- The secret redaction MUST get the provider keys by calling the provider-key export.
- The consent gate MUST ask for consent only when the resolved model value's provider is the wheel's TypeSafe provider constant (`sc_agent.TYPESAFE`); a refused consent MUST send nothing.


- Every application record written to a general log MUST carry `schema`, `tool`, `tool_version`, `run_id`, `project` and `agent_role` from its Run. Records tied to one query MUST carry that query's `session_id`; non-query records omit it. A batch result that failed before a session exists retains the existing nullable `session_id` field of `jev-batch-result/2`.
- The Rust Run MUST own construction of that header; the generated client MUST obtain it through the wheel. Tool-specific payloads stay in tool records, with no second copy of request, answers, attempts, sends or model response (NFR-SAG-0004).
- `jev-batch-result/2` MUST remain the batch-result tag; there is no second batch schema bump in phase k.
- One shared `log_result_line` in the generated client MUST write generic batch-result events for both tools. compare, drift and plan-anchor run summaries MUST use the same header with their own schema tags and tool-owned derived fields.
- The client MUST provide the linked walk records of REQ-SAG-0024 for any injected decision tree and branch-selection callback. Prompt authoring, branch selection and verdict interpretation stay with the tool.
- Dry-run commands MUST continue to compose and display requests without creating a Run or writing any session, run, result, walk or general-log application record.

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

The owner requires judge-jev, plan-anchor and every later Python consumer to integrate identically for errors, API keys, model selection (the jev, haiku, luna, decider and laya alternates), the injectable general logger and session logging. They differ only in prompt authoring and in processing results and decision trees. Session logging is always on and always at the wheel's default location, so every consumer's Jev data takes the same path and can be filtered by repository and agent. A hand-copied recipe already diverged between two consumers (ADR-SAG-0019, Context). One template rendered by every consumer removes that divergence. Shipping the template as package data makes its version the installed wheel's version. Leaving the provider to resolution keeps the jev, haiku, luna, decider and laya alternates available.

Owner ruling (2026-11-20), Rand Lee: "Keyring is super basic."

Owner ruling (2026-11-21), Rand Lee: the shared template carries what the wheel does not support and both consumers share (fixture loading and coverage, batch settings and ids, result-line encoding, attempt counting, the remote-judge consent gate), with consumer-specific values only through template variables or named blocks, as extension points for future consumers.

Owner ruling (2026-11-21), Rand Lee: "No hand built checks." "Keyring is super basic." "always return descriminated unions w/ error info"

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

Owner rulings (2026-11-24), Rand Lee, recorded in `docs/plans/phase-j/audit-trail-log-normalization-SQLite.md`: a tool references `session_id` and never copies the request or the response; the shared result line holds generic fields only and tool ids live in tool records; `run_id` is a ULID minted once per run; a dry run writes nothing as if it were real. A `session_id` is known only after the send returns, and a cache hit returns an older one.

Owner rulings (2026-11-24, round 3), Rand Lee, recorded in the same design doc: the session row carries no `run_id`; the template passes `run_id` to every send, and sc-agent appends a `{run_id, session_id, cached}` link record to `runs.jsonl` on a cache miss and on a cache hit, so the run log plus the tool records hold the link from a run to its queries. Every tool built on sc-agent works with `--model haiku` and `--model luna`, including when no `TYPESAFE_API_KEY` is set, and secret redaction covers every provider key. Resuming a haiku or luna session to ask follow-up "why" questions is future work, GitHub issue randlee/sc-agent#428, and is not required here.

Owner rulings 2026-11-24 (plan review, ADR-SAG-0003): no copying, every piece of logic or data has one owner and the consumer calls it (NFR-SAG-0004). A run is a context object, `sc_agent.Run`, created once per command and closed when its calls are done; creating it mints `run_id` in Rust, and it carries the tool identity, the cache policy, the resolved configuration and the Logger, so calls through it take no `run_id`, `tool` or `tool_version`. A model selection is resolved once into a typed model value that carries the provider; there is no `provider` argument. The wheel owns ULID minting, the session-row reader and the provider-key list, which covers every provider in the Rust table. One cache policy per call; a caller needing both sends two batches.

Owner ruling 2026-11-24: allow_remote_judge is consent for TypeSafe (Jev) only; haiku, luna and the local endpoints are not gated by it.

Lead ruling 2026-11-24 (plan review): a fixture run answers in process and opens no Run, so it writes nothing a live run would; a Run that failed creation has no `run_id`, so its result lines carry null.

Lead ruling (2026-11-24, round 3): attempt counting is removed from the shared consumer code; `attempts` is session data, read from the session row.

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
- [ ] A `KeyboardInterrupt` during a rendered module's batch cancels its Run, and the batch returns cancelled results.
- [ ] Scanning both rendered modules finds only wheel references to the permitted API: `Run`, `Model`, `TestSeams`, `TYPESAFE`, `session_rows`, `provider_keys`, `digest`, `logger`, `log`, `flush` and `sc_agent.sitr`.
- [ ] The README of `crates/sc-agent-py` names the template and its render command.
- [ ] A rendered module's batch against the loopback runner writes one `jev-batch-result/2` line per query, each with exactly the generic result fields, one shared ULID `run_id` equal to the Run's `run_id`, and a `session_id` that `session_rows` finds in the session log.
- [ ] Two runs of the same command carry different `run_id`s.
- [ ] With the cache on, a second identical run writes result lines whose `session_id`s equal the first run's, under the second run's `run_id`, and the session log gains no row.
- [ ] A rendered module's call with `cache=False` adds a new session row for an identical request.
- [ ] A rendered module's dry run prints the would-send requests to stdout and, separately, to an `--out` file; it creates no Run, writes no result line, no tool record and no general-log event carrying session data, and adds no session row or link record.
- [ ] A rendered module's batch of 3 queries under `run_id` R adds 3 link records with `run_id` R to the run log, and their `session_id`s equal the 3 result lines' `session_id`s.
- [ ] With `TYPESAFE_API_KEY` unset and only `ANTHROPIC_API_KEY` set to a dummy value, each rendered module's `--model haiku` call completes against the loopback provider stub.
- [ ] With `TYPESAFE_API_KEY` unset and only `OPENAI_API_KEY` set to a dummy value, each rendered module's `--model luna` call completes against the loopback provider stub.
- [ ] With every provider key that `provider_keys()` returns set to a distinct dummy value, a rendered module's general log, result lines, dry-run output and stdout contain none of those values.
- [ ] A contract test shows `provider_keys()` equals the credential names of the Rust provider table, and a scan of both rendered modules finds no provider-key name, no ULID minting and no session-log path literal.
- [ ] A rendered module's call whose model value's provider is TypeSafe asks the consent block, and a refusal sends nothing; a `haiku` call configured to a non-TypeSafe provider sends without asking.
- [ ] A rendered module's fixture run creates no Run and adds no result line, session row or link record.
- [ ] Result lines written through a Run that failed creation carry `run_id` null.
- [ ] With a provider key set to a dummy value and a `Bearer <token>` in an error text, a rendered module's output shows `[redacted]` for both.
- [ ] A result line's `project` equals the Run's resolved project/phase when the project came from configuration.

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
