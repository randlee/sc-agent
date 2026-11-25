# Decision: sc-decidr is the one Python decision harness: error values with a stored category, per-call limits and a generated consumer module

**Status:** Approved  
**Version:** 1.5.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-14  

---

## ADR-SAG-0019: sc-decidr is the one Python decision harness

**Status:** Approved  

### Related Documents

New with this ADR: REQ-SAG-0016 (typed error fields), REQ-SAG-0017 (the category table), REQ-SAG-0702 (per-call limits), REQ-SAG-0018 (the generated consumer module), and NFR-SAG-0003 (one Python floor). REQ-SAG-0007 covers decision values. REQ-SAG-0601 and REQ-SAG-0602 cover resends. REQ-SAG-0701, REQ-SAG-0013 and REQ-SAG-0015 set the configured worker count and timeouts. ADR-SAG-0004 sets the crate layering and that errors are `Result<T, E>`. Amended 2026-11-24 (decisions 1, 13, 14 and 15): REQ-SAG-0005 and REQ-SAG-0703 (session rows, run log and result cache), REQ-SAG-0101, REQ-SAG-0102, REQ-SAG-0801 and REQ-SAG-0019 (providers as endpoints, model rows and model configuration), REQ-SAG-0022 (the session-row reader) and NFR-SAG-0004 (one owner). ADR-SAG-0021 sets the log paths and the session sink. Amended 2026-11-24 (decisions 1 and 14, plan review round 2): the render variables, the result types inside the consumer template, the consent settings module, the complete category-to-code map, the `sc_agent.testing` module and the `config.missing_project` error id.

**Requirements:**

- REQ-SAG-0016
- REQ-SAG-0017
- REQ-SAG-0702
- REQ-SAG-0018
- NFR-SAG-0003
- REQ-SAG-0007
- REQ-SAG-0002
- REQ-SAG-0601
- REQ-SAG-0602
- REQ-SAG-0701
- REQ-SAG-0013
- REQ-SAG-0015
- REQ-SAG-0005
- REQ-SAG-0703
- REQ-SAG-0101
- REQ-SAG-0102
- REQ-SAG-0801
- REQ-SAG-0019
- REQ-SAG-0022
- NFR-SAG-0004
- NFR-SAG-0005

**Architecture Decisions:**

- ADR-SAG-0004
- ADR-SAG-0021

### Context

sc-decidr is the one harness through which sc-agent's Python consumers reach decision models: Jev, decider, Laya, Haiku and Luna. It owns model selection, API-key lookup, session logging (one row per query sent to a model, REQ-SAG-0005), the injectable general logger, resends and errors.

The owner requires that judge-jev, plan-anchor and every later Python consumer have identical integration for errors, API keys, model selection, the injectable general logger and session logging. They differ only in prompt authoring and in processing results and decision trees.

The two existing consumers each rebuilt the wheel boundary, and the copies diverged:

- **Errors.** judge-jev pins the original id list and falls back to `JEV.UNMAPPED`. plan-anchor matches message text (`"http 401"`).
- **Model.** judge-jev passes the model selector unchanged. plan-anchor pins `provider="typesafe"`, which disables Haiku and Luna.
- **Order.** Both re-sort `send_batch` results into input order.
- **Timeout.** judge-jev refuses `--timeout`. plan-anchor calls the unreleased `timeout_secs`.

Two gaps remained: consumers had no category to branch on, so they pinned id lists; and there was no per-call worker count or CLI timeout flag.

### Decision

sc-decidr makes the wheel the one consumer boundary for Python decision consumers, through these decisions. Decision numbers are stable; there is no decision 3.

1. **The wheel is the consumer boundary.**

   Every consumer concern except prompt authoring and result interpretation belongs in sc-decidr.

   | Concern | Provided by sc-decidr | Consumer does |
   | --- | --- | --- |
   | Model and provider | A model selector (short name or full model id) is resolved once, by decision 13, into a typed model value held by the Run (`model=`); one model per Run, chosen by the caller. There is no `provider` argument. | Pass the selector unchanged. |
   | API keys | The environment, then the repo-root `.env`, then the OS keyring. | Nothing. |
   | Session log | One row per query sent to a model, always written to the wheel's default location, tagged with `project` and `agent_role`; a cache hit writes no row and one link record in `runs.jsonl` (REQ-SAG-0005, REQ-SAG-0703). | Pass `project` and `agent_role`. Rows are written by sc-agent's session sink (ADR-SAG-0021); no session Logger is injected. |
| Run | `sc_agent.Run(tool=, tool_version=, cache=, ...)`, a context object that mints the run's ULID `run_id` in Rust, resolves the configuration once and owns the Logger, the cancel flag and the cache policy (decision 14). | Create one Run per command, send through it, read `run.run_id` for its own records. |
| Session rows | `sc_agent.session_rows`, the one reader of the session-log layout (REQ-SAG-0022). | Call it; never read session files itself. |
| Provider keys | `sc_agent.provider_keys()`, the credential names of every provider in the Rust provider table. | Call it for secret redaction; never list keys itself. |
   | General log | The generated client obtains one Logger from the sc-decidr wheel, named after the client and writing `~/.sc-agent/logs/<client>.log.jsonl`, gives it to its Run at creation and injects it into every sitr call (REQ-SAG-0022, ADR-SAG-0021). | Log its own records to the client's Logger, or pass its own Logger to the client. |
   | Errors | The returned error value: `id`, `message`, `action`, a stored `category` and the send count (decisions 2, 5 and 6). | Map a category to its own codes. Never parse message text. |
   | Limits | `timeout_secs`, `workers`, a fixed resend count and cancellation, set on the Run (decisions 7 and 14). | Pass its own flags through. |
   | Batch order | `send_batch` returns one result per request, each with its input position (decision 8). | Nothing. |

   **The integration artifact is a generated module** (REQ-SAG-0018). The wheel ships an sc-compose template, `sc_agent/templates/consumer.py.j2`, as package data, and each consumer renders its integration module from it. To carry the file, the wheel moves to maturin's mixed layout; `import sc_agent` is unchanged.

   - The template covers the decision call, model pass-through, logging wiring, cancellation, the error-to-category-to-code mapping and the CLI envelope helpers. Prompt authoring and the consumer's domain handling of results stay in the consumer.
   - Its variables are `client_name`, `tool_version`, `sitr` (`true` or `false`), `sc_decidr_version` and `legacy_result_schemas`; model selectors are call arguments. `legacy_result_schemas` is the list of result-line schema tags, besides `jev-batch-result/1`, whose lines the module still reads; it is empty for a consumer that has none.
   - The result types `Ok`, `Err` and `Result` and the CLI envelope helpers `ToolError`, `err`, `utf8_stdio`, `canceled_envelope` and `run_main` are defined inside `consumer.py.j2`. The wheel ships no other template for the consumer module.
   - The remote-judge consent setting is read by the consumer's own consent settings module, which the template names through an extension point. The template contains no settings reader.
   - The template's category-to-code map has an entry for every category of decision 6. A category outside that list maps to the consumer's generic error code, the code the map gives for `other`. Session logging and the injected general logger are not optional (REQ-SAG-0008, REQ-SAG-0022).
   - sc-compose is an external render tool, not a dependency of the wheel. The template's front matter declares its sc-compose minimum (`requires.cli`); the minimum is declared, not enforced.
   - All Jev queries and responses go to the session log, so every consumer's Jev data takes the same path and can be filtered by repository and agent.

   **The lower-level API stays the contract.** `Run` (with its `send` and `send_batch`), `session_rows`, `provider_keys`, `digest`, `logger`, `log`, `flush` and `sc_agent.sitr` are public. The generated code uses only them and the standard library.

   **`sc_agent.testing` is a public test module of the wheel.** It holds `FakeRun`, a stand-in for a Run that records the arguments it was opened with and the requests sent through it and returns scripted results, and `fake_open_run(script)`, which installs a `FakeRun` for a rendered module and answers that module's `session_rows` calls from the script's rows, for consumers' tests. The generated code does not import it.

2. **Typed fields: `attempts` and `category`.**

   An error value carries at least `id`, `message` and `action`, and the typed fields below when they apply. It may carry more fields. Unset is not written.

   | Field | Meaning |
   | --- | --- |
   | `attempts` | The sends made for this request, the first one included. |
   | `category` | The error's category (decision 6). |

   REQ-SAG-0016 holds the requirements.

   **Retry-After is read as a Duration** and used for the wait before a resend.

4. **Breaking minor release.**

   The Rust change breaks SemVer: the error type and the call API change. Below 1.0 a minor bump is the breaking bump. For Python the change only adds things.

   All workspace crate and wheel versions move together (`[workspace.package].version` in `Cargo.toml`); `verify-version-lockstep` enforces this. sc-decidr is the one wheel and holds the sitr API, so decidr and sitr always have the same version in Python.

5. **Python errors.**

   Every wheel call returns ok with a value or an error with its id and message; nothing raises.

6. **The category is stored when the error is built.**

   The categories are `busy`, `quota`, `auth`, `network`, `response_invalid`, `validation`, `cancelled`, `config`, `log` and `other`.

   - Every error has a stable id.
   - Every error stores its category.

   Python reads the `category` field of the returned error value.

   **Retryable means "worth resubmitting as a new call later, after the wheel's own resends have run out."** It is true for `busy` and `network` only. A consumer that resubmits waits first and does not wrap the wheel in an immediate retry loop.

7. **Per-run limits: `timeout_secs` and `workers`.**

   A Run takes an optional timeout (a Duration) and an optional worker count; each applies to that Run's calls only; a bad value is an error value. Amended 2026-11-24: the limits moved from each call to the Run (decision 14).

   - Python: `Run(workers=, timeout_secs=)`. CLI: `--workers N` and `--timeout-secs SECS`, for that command's Run.
   - A limit applies to its own Run only. `None` means the configured value. A `workers` value below or above the configured count is honored.
   - `timeout_secs` replaces the default 120 s send timeout for every send of the Run.
   - Every send gets the full timeout, resends included. Busy and network errors are resent up to a fixed count.

   REQ-SAG-0702 holds the requirements.

8. **`send_batch` returns one result per request, each with its input position.**

   `on_result` still receives each line as it finishes.

9. **Verification is the contract tests and the consumers' suites; the integration gate is removed.**

   `decidr::integration_gate`, its re-exports `integration_cases` and `validate_integration_results`, the matching `sc_agent` functions and the surrounding report and gate tooling are removed before 0.2.0. `decidr.integration_evidence` is not an error id.

   Contract tests run at the product boundary against a local HTTP server in the test, with no network, no real home and no sleeps. The work is done when they pass and plan-anchor's and judge-jev's suites pass against the new wheel. REQ-SAG-0133 is superseded by this decision.

10. **Category assignments are not locked.**

    Rand Lee (2026-11-20): "schema isn't locked yet ... No hand built checks."

11. **sitr is an optional, separate section of the consumer template.**

    sitr's Python API is `sc_agent.sitr`, inside the sc-decidr wheel (ADR-SAG-0005, ADR-SAG-0021). The template's sitr section is kept separate from the decision call: extraction runs outside the decision path, and its output is passed into the consumer's prompt authoring. The section uses `sc_agent.sitr` only. In this phase one offline test shows extraction output reaching a dry-run request; sitr is not used beyond that yet.

12. **One Python floor (NFR-SAG-0003).**

    - `floors.toml` at the repository root writes down the Python floor (`[python] floor`) and the sc-compose minimum (`[tools] sc-compose`). It only records values.
    - The pyo3 abi3 feature in `sc-agent-py` moves to the abi3 feature of the Python floor, so the wheel tag matches the `requires-python` the wheel declares.
    - Until sc-compose can read `floors.toml`, the template carries these values as literals.

13. **A provider is an endpoint; models are rows that reference it.**

    - A provider is a base address, a wire and an optional credential name. The base address is a fixed remote URL or a host and port read from configuration (the local endpoints `laya` and `decider`).
    - The model table has one row per model: short name, model id, release date and the provider it references. Every row has a fixed model id, local endpoints included: laya's is `laya:en`, and decider has two rows, `decider-2b-v11` and `decider-4b-v2.1`, each on its own local endpoint, so `decider` resolves by the same rule as `haiku`. Every request carries the resolved id as `model`; the decider servers accept and ignore it.
    - sc-agent never branches on a provider's or a model's name. Adding a model is one row plus configuration.
    - Configuration key `models.<alias>` with `provider` and `id` overrides the built-in row for that alias (REQ-SAG-0019); it is not how a local model gets its id. It may only name a built-in provider; it cannot add a provider, a credential or a remote address.
    - The model table is sorted by release date, newest first, and a unit test verifies the order.
    - Resolution, with no `provider` argument: the configured `models.<alias>.provider` when set; otherwise the first row in table order that serves the alias and whose provider is keyless or has its credential in a source of REQ-SAG-0401. When none qualifies and a candidate's keyring read failed, resolution returns that keyring error (a `secrets.*` id); otherwise, when none qualifies, the error names every candidate's credential.
    - A full model id selects exactly the row with that id.
    - A model selection is resolved once, at the boundary (a CLI flag or Run creation), into one typed value, `ResolvedModel { alias, id, provider }`, where `provider` references the provider constant (base address, wire, credential). That value is what flows through the send, the cache key, the session row's `model_requested` and `provider`, and the redaction; no model name and provider name pair is passed down the stack or resolved again. There is no `provider` argument or `--provider` flag: `models.<alias>.provider` in configuration and the rule above choose the provider. In Python the Run holds its model value, readable as the property `run.model`; there is no per-call model and no selector method. A caller that needs another model opens another Run. Consumers take the selector from their caller (`--model` or configuration) and never name a model in code; fan-out across models is the caller's job (owner ruling 2026-11-24).
    - The haiku and luna rows include Nous: `anthropic/claude-haiku-5.5` and `openai/gpt-6-luna`.

14. **A run is a context object.**

    - `sc_agent.Run(tool=, tool_version=, cache=, ...)` is created once per tool command and closed when its calls are done (`with sc_agent.Run(...) as run:`). Creating it mints the run's ULID `run_id` in Rust; `session_id` ULIDs are also minted in Rust. `run.run_id` is readable for the tool's own records.
    - Everything that does not change between the calls of one command is resolved once, when the Run is created, and held by it: the tool identity, the cache policy, the project/phase, the configuration directory and the layered settings, the worker count, the send timeout, the repository identity, the log root and the credential sources. The Run owns the general Logger, or borrows the Logger given at creation; it flushes after each call and on exit.
    - Per call, a caller passes only what may vary between calls: the request or requests, and optionally `agent_role`, a `cache` override for that call, and `on_result` for a batch. The model is set on the Run; a default `agent_role` may be.
    - `run.cancel()` cancels the Run's calls. A call after the Run is closed returns a typed error value.
    - Creating a Run never raises. When a creation value is invalid (an empty `tool` or `tool_version`, a bad limit, an unresolvable model), the Run holds that error value, and each of its calls returns it with no send, no session row and no link record. A missing project/phase is not a creation error: each call returns `config.missing_project`.
    - In Python, the Run's keyword arguments are read through one keyword catch-all, so an argument of the wrong type or an unknown keyword (including `provider=`) is held or returned as `sc_agent_py.invalid_option` and does not raise (owner ruling 2026-11-24: PyO3 calls never raise).
    - One send path: in Rust, `Run::send` and `Run::batch` both call one private sender; in Python, `run.send` is `run.send_batch` with one request (Owner ruling 2026-11-24: calls go through the Run (`run.send` / `run.send_batch`)).
    - There is no per-job cache: a caller that needs both policies sends two batches; each job is still looked up on its own (REQ-SAG-0703).
    - The test seams (provider base URL, the wait function and the keyring lookup) are given once, when the Run is created, as one typed value; they are not call arguments.
    - In Rust the Run is one struct holding the resolved values above; the run's ULID, the tool identity and the cache policy are plain fields, since a single value needs no newtype (NFR-SAG-0005). `Run::send` and `Run::batch` are methods that take the per-call values (Owner ruling 2026-11-24: calls go through the Run (`run.send` / `run.send_batch`)), and `decidr` creates one Run per command. In Python the Run and the Logger are `#[pyclass]` types with typed fields; no capsule and no string-keyed option table crosses the boundary. The Python Run's exit flushes the Logger.

15. **One owner (NFR-SAG-0004).**

    ULID minting, the provider table and its credential list, the model table and the session-log layout are owned by the Rust crate and exported by the wheel; the consumer template and every consumer call them and hold no copy.

16. **Phase-k audit values share their owners.** Run lifecycle events are methods of the existing Run, not a second lifecycle or free `run_start`/`run_end` API (REQ-SAG-0023). Common application headers and walk collections are created by Rust and exposed as typed pyclasses; walk ids are ulid::Ulid in Rust and strings at the Python boundary, without single-value wrappers (NFR-SAG-0005). Provider answers, persisted typed answers and Python answers use the same per-kind Rust types (REQ-SAG-0005); the template's duplicate NamedTuple answer shapes and kind-inference parser leave. The planned session tag stays `sc-agent-session/2`; its earlier opaque-result rows remain readable by shape without a historical rewrite. The batch-result tag stays /2. The client template owns generic walk scheduling and logging; consumer callbacks own prompt and branch semantics (REQ-SAG-0024). SQLite, importers and HTML reports belong to the later design phase.

### Rationale

Two consumers rebuilt the wheel boundary and the copies diverged, so every concern except prompt authoring and result interpretation moves into sc-decidr, and the integration code is generated from one template. A stored category and the send count replace message-text parsing and pinned id lists. Storing the category when the error is built needs no lookup. Per-call limits and position-tagged batch results remove the consumer code that worked around their absence. Prerelease tooling leaves the product API so it does not freeze into the error contract.

Owner ruling 2026-11-21: records follow docs/plans/reduction-plan.md.

Owner ruling 2026-11-24 (plan review, ADR-SAG-0003): this ADR is amended in place. Every piece of logic or data has one owner and everything else calls it. A provider is an endpoint and models are table rows that reference provider constants, with no name-string branching; laya and decider are models on configured local endpoints, and decider's model id is optional. The run is a typed context object that mints `run_id` in Rust; everything that does not vary between its calls (configuration, project, repository, log root, Logger, cancel flag, limits, test seams) is resolved once at its creation, and per call only the request, model, agent role, cache override and batch callback are passed. Values passed together are one Rust struct (NFR-SAG-0005). The wheel owns ULID minting, the session-row reader and the provider-key list. One cache policy per call, no per-job cache list. A model selection is resolved once at the boundary into one typed value that carries the provider; there is no separate `provider` argument. Session logging writes one row per query sent to a model; a cache hit writes a link record and no row.

Owner ruling 2026-11-24 (supersedes the optional or configured local model id and the omitted `model` above): laya and decider have fixed model ids in the model table, like every other model. laya requires `laya:en`. decider has two rows, `decider-2b-v11` (port 18000) and `decider-4b-v2.1` (port 18001), both released 2026-09-24, and `decider` resolves by the same rule as `haiku`. The decider servers accept and ignore `model`, so every request carries the resolved id. Cache keys always use the resolved fixed id.

Owner ruling 2026-11-24 (plan review), on resolving a short name to the first row in table order whose provider is keyless or holds a credential: "that is fine as long as the table is sorted (newest-oldest) and verified to be sorted in unit test." Owner ruling 2026-11-24: consumers do not hard-code model names (`sc_agent.model("haiku")` is a "red flag for inefficient/hardcoded inflexible design"); a caller runs jev or haiku or luna or laya or decider, one model per Run. Owner ruling 2026-11-24: `decider` defaults to 4b; `decider-4b-v2.1` is the decider row listed first, so `decider` resolves to it.

Owner ruling 2026-11-24: calls go through the Run (`run.send` / `run.send_batch`): decision 14's send entry points are Run methods over one private sender. Lead ruling 2026-11-24: single values are plain fields rather than newtypes (NFR-SAG-0005 covers collections of two or more values). Owner ruling 2026-11-24: PyO3 calls never raise; an unknown or mistyped keyword, read through the `**rest` catch-all or an `Option<&Bound>` argument, returns `sc_agent_py.invalid_option`.

Lead ruling 2026-11-24 (plan review round 2): `Ok`, `Err` and `Result` move into `consumer.py.j2` instead of a sibling template; each consumer keeps its consent settings reader, so no settings template or YAML parser enters the wheel; `sc_agent.testing.FakeRun` replaces each consumer's own Run double.

Owner ruling (2026-11-21), Rand Lee: "I would never create a requirement requiring subnanosecond rounding for a http call." "Use time structs for time." "always return descriminated unions w/ error info" "I do not require results in request order." "No hand built checks."

### Consequences

**Positive:**

- Every Python consumer renders the same integration module and contains only its own prompts and result handling.
- No consumer parses error text or pins id lists.
- All Jev data from every consumer takes one session-log path.
- Prerelease tooling no longer ships in the product crate or wheel.

**Negative:**

- This is a breaking release for Rust dependents: the error type and the call API change.
- The wheel moves to maturin's mixed layout.

### Alternatives Considered

#### Alternative 1: A separate `server` category for 500, 503 and 504

**Description:**

Add a `server` category instead of counting these final replies as `busy`.

**Why Rejected:**

It would extend the fixed category set for a distinction no consumer acts on; `busy` already means "resubmit later".

#### Alternative 2: A "Consumer integration" recipe in the README

**Description:**

Document the integration as a recipe for each consumer to copy.

**Why Rejected:**

A recipe is copied by hand, and two hand-written copies already diverged.

#### Alternative 3: An import-time version check in the consumer module

**Description:**

Raise on import when the versions recorded in the generated header differ from the installed sc-decidr or sitr.

**Why Rejected:**

A fail-fast check makes troubleshooting and development harder. Mixed versions are documented as unsupported, not enforced.

### Implementation

The work lands on one branch, `rewrite/v1`, and merges into `develop` in one PR. The tests are the acceptance criteria of REQ-SAG-0016, REQ-SAG-0017, REQ-SAG-0018, REQ-SAG-0702 and NFR-SAG-0003.

Deliverables beyond the code:

- `CHANGELOG.md` (new), with "Breaking" and "Consumer migration" sections. The migration section names what judge-jev and plan-anchor delete: pinned id lists, `JEV.UNMAPPED`, message-text status parsing, the `--timeout` refusal, the ignored `--max-workers`, the `provider="typesafe"` pin, result re-sorting, their own retry loops and their hand-written integration glue.
- The consumer template, `sc_agent/testing.py` and the `sc_agent/__init__.pyi` stub.
- `floors.toml`.
- The sc-decidr wheel, holding the decidr and sitr APIs, released to PyPI.

Changes inside judge-jev, plan-anchor and other consumer repositories are out of scope.
