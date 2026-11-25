# Decision: sc-decidr is the one Python decision harness: error values with a stored category, per-call limits and a generated consumer module

**Status:** Approved  
**Version:** 1.6.1  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-14  

---

## ADR-SAG-0019: sc-decidr is the one Python decision harness

**Status:** Approved  

### Related Documents

New with this ADR: REQ-SAG-0016 (typed error fields), REQ-SAG-0017 (categories), REQ-SAG-0702 (limits and batch order), REQ-SAG-0018 (the generated consumer module) and NFR-SAG-0003 (one Python floor). Decisions 13 to 15 were added, and decisions 1 and 14 amended, on 2026-11-24: model resolution is REQ-SAG-0101, REQ-SAG-0102, REQ-SAG-0103 and REQ-SAG-0801, configuration is REQ-SAG-0019, session rows, run log and cache are REQ-SAG-0005 and REQ-SAG-0703, the session-row reader is REQ-SAG-0022, and one owner is NFR-SAG-0004. ADR-SAG-0004 sets the crate layering and that errors are `Result<T, E>`; ADR-SAG-0021 sets the log paths and the session sink. Implementation detail is in the phase-j plan beads agt-j-1 (Run, providers, PyO3) and agt-j-3 (template, `sc_agent.testing`).

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
- REQ-SAG-0103
- REQ-SAG-0801
- REQ-SAG-0019
- REQ-SAG-0022
- NFR-SAG-0004
- NFR-SAG-0005

**Architecture Decisions:**

- ADR-SAG-0004
- ADR-SAG-0021

### Context

sc-decidr is the harness through which sc-agent's Python consumers reach decision models (Jev, decider, Laya, Haiku, Luna). The owner requires judge-jev, plan-anchor and every later consumer to integrate identically for errors, API keys, model selection, the injectable general logger and session logging, differing only in prompt authoring and result handling.

The two existing consumers each rebuilt the wheel boundary, and the copies diverged: judge-jev pins an id list with a `JEV.UNMAPPED` fallback while plan-anchor matches message text (`"http 401"`); plan-anchor pins `provider="typesafe"`, disabling Haiku and Luna; both re-sort `send_batch` results; judge-jev refuses `--timeout` while plan-anchor calls the unreleased `timeout_secs`. Consumers had no category to branch on and no per-call worker count or timeout.

### Decision

The wheel is the one consumer boundary. Decision numbers are stable; there is no decision 3.

1. **The wheel is the consumer boundary.** Every consumer concern except prompt authoring and result interpretation belongs in sc-decidr: model selection (decision 13), API keys (environment, repo-root `.env`, OS keyring), session logging, the general Logger, errors with a stored category (decisions 2, 5 and 6), limits and cancellation (decisions 7 and 14) and position-tagged batch results (decision 8). Neither session logging nor the injected general logger is optional (REQ-SAG-0008, REQ-SAG-0022). Session logging is always on: one row per query sent to a model, at the wheel's default location, tagged with `project` and `agent_role`, written by sc-agent's session sink (ADR-SAG-0021); a cache hit writes no row and one link record (REQ-SAG-0005, REQ-SAG-0703). All Jev queries and responses therefore take one path. The generated client gets one Logger from the wheel, named after the client and writing `~/.sc-agent/logs/<client>.log.jsonl`, and gives it to its Run and every sitr call (REQ-SAG-0022).

   **The integration artifact is a generated module** (REQ-SAG-0018), rendered by each consumer from the sc-compose template `sc_agent/templates/consumer.py.j2`, shipped as package data; the wheel moves to maturin's mixed layout and `import sc_agent` is unchanged.

   - The template covers the decision call, model pass-through, logging wiring, cancellation, the error-to-category-to-code mapping and the CLI envelope helpers.
   - Its variables are `client_name`, `tool_version`, `sitr` (`true` or `false`), `sc_decidr_version` and `legacy_result_schemas`, the result-line schema tags besides `jev-batch-result/1` that the module still reads (empty when none); model selectors are call arguments.
   - The result types `Ok`, `Err` and `Result` and the CLI envelope helpers `ToolError`, `err`, `utf8_stdio`, `canceled_envelope` and `run_main` are defined inside `consumer.py.j2`. The wheel ships no other template for the consumer module.
   - The consumer's own consent settings module reads the remote-judge consent setting and is named through an extension point; the template contains no settings reader.
   - The category-to-code map has an entry for every category of decision 6; any other category maps to the code for `other`, the consumer's generic error code.
   - sc-compose is an external render tool, not a wheel dependency; the template's front matter declares its sc-compose minimum (`requires.cli`), declared, not enforced.

   **The lower-level API stays the contract.** `Run` (with `send` and `send_batch`), `session_rows`, `provider_keys`, `digest`, `logger`, `log`, `flush` and `sc_agent.sitr` are public, and the generated code uses only them and the standard library.

   **`sc_agent.testing` is a public test module.** `FakeRun` stands in for a Run: it records its open arguments and the requests sent through it and returns scripted results. `fake_open_run(script)` installs a `FakeRun` for a rendered module and answers its `session_rows` calls from the script. `seed_session(log_root, request, answer, **run_options)` writes one successful session row and its run link through the real session writer, so a real Run opened on that log root with the same options and the cache on answers the request from the cache with no network call. The generated code does not import it (agt-j-3 J3-16).

2. **Typed fields: `attempts` and `category`.** An error value carries at least `id`, `message` and `action`, plus `attempts` (sends made, the first included) and `category` (decision 6) when they apply; unset fields are not written (REQ-SAG-0016). Retry-After is read as a Duration and used for the wait before a resend (REQ-SAG-0601).

4. **Breaking minor release.** The error type and call API change, which breaks SemVer for Rust; below 1.0 a minor bump is the breaking bump, and for Python the change only adds things. All workspace crate and wheel versions move together (`[workspace.package].version` in `Cargo.toml`), enforced by `verify-version-lockstep`; sc-decidr is the one wheel and holds the sitr API, so decidr and sitr always share a version.

5. **Python errors.** Every wheel call returns ok with a value or an error with its id and message; nothing raises (REQ-SAG-0016).

6. **The category is stored when the error is built.** The categories are `busy`, `quota`, `auth`, `network`, `response_invalid`, `validation`, `cancelled`, `config`, `log` and `other`. Every error has a stable id and stores its category, which Python reads from the `category` field (REQ-SAG-0017). Retryable means worth resubmitting as a new call later, after the wheel's own resends have run out; it is true for `busy` and `network` only, and a consumer that resubmits waits first rather than wrapping the wheel in an immediate retry loop.

7. **Per-run limits: `timeout_secs` and `workers`.** A Run takes an optional timeout (a Duration) and worker count, for that Run's calls only (`--timeout-secs` and `--workers` on the CLI); `None` means the configured value, and every send, resends included, gets the full timeout. The limits moved from each call to the Run on 2026-11-24 (decision 14). REQ-SAG-0702 holds the rules.

8. **`send_batch` returns one result per request, each with its input position**, while `on_result` still receives each line as it finishes (REQ-SAG-0702).

9. **Verification is the contract tests and the consumers' suites; the integration gate is removed.** `decidr::integration_gate`, its re-exports `integration_cases` and `validate_integration_results`, the matching `sc_agent` functions and the report and gate tooling are removed before 0.2.0; `decidr.integration_evidence` is not an error id. Contract tests run at the product boundary against a local HTTP server, with no network, no real home and no sleeps; the work is done when they and plan-anchor's and judge-jev's suites pass against the new wheel. REQ-SAG-0133 is superseded.

10. **Category assignments are not locked** (owner ruling 2026-11-20).

11. **sitr is an optional, separate section of the consumer template.** It uses `sc_agent.sitr` only (ADR-SAG-0005, ADR-SAG-0021) and is kept separate from the decision call: extraction runs outside the decision path, and its output feeds the consumer's prompt authoring. In this phase one offline test shows extraction output reaching a dry-run request.

12. **One Python floor (NFR-SAG-0003).** `floors.toml` at the repository root records the Python floor (`[python] floor`) and the sc-compose minimum (`[tools] sc-compose`); the pyo3 abi3 feature of `sc-agent-py` follows the floor so the wheel tag matches its `requires-python`. Until sc-compose can read `floors.toml`, the template carries these values as literals.

13. **A provider is an endpoint; models are rows that reference it.** A provider is a base address (a fixed URL, or a configured host and port for the local `laya` and `decider` endpoints), a wire and an optional credential name; the model table has one row per model with a fixed model id, local endpoints included, and sc-agent never branches on a provider's or model's name, so adding a model is one row plus configuration. Configuration `models.<alias>` entries add rows, and short names resolve over the merged table sorted newest first (REQ-SAG-0102, REQ-SAG-0103, REQ-SAG-0019; local endpoints: REQ-SAG-0801). A local reply whose `model` differs from the resolved id is `decision.model_mismatch` and writes no session row and no run link. A selection is resolved once, at the CLI flag or Run creation, into one typed value, `ResolvedModel { alias, id, provider }`, which flows through the send, the cache key, the session row's `model_requested` and `provider`, and the redaction; there is no `provider` argument or `--provider` flag. The Python Run holds its model as `run.model`, with no per-call model: a caller that needs another model opens another Run, consumers take the selector from their caller (`--model` or configuration) and never name a model in code, and fan-out across models is the caller's job (agt-j-1 J1-6, J1-12).

14. **A run is a context object.** `sc_agent.Run(tool=, tool_version=, cache=, ...)` is created once per tool command and closed when its calls are done (`with sc_agent.Run(...) as run:`); creating it mints the run's ULID `run_id` in Rust (as are `session_id` ULIDs), readable as `run.run_id`.

    - Everything that does not change between a command's calls is resolved once at creation and held by the Run: tool identity, cache policy, project/phase, configuration and layered settings, worker count, send timeout, repository identity, log root, credential sources and the test seams (one typed value; not call arguments). The Run owns the general Logger or borrows the one given, and flushes after each call and on exit.
    - The model, and optionally a default `agent_role`, are set on the Run. Per call a caller passes only the request or requests and optionally `agent_role`, a `cache` override and, for a batch, `on_result`; there is no per-job cache (REQ-SAG-0703). `run.cancel()` cancels the Run's calls; a call after close returns a typed error value.
    - Creating a Run never raises: an invalid creation value is held and returned by each call with no send, no session row and no link record; a missing project/phase is instead `config.missing_project` per call. In Python an unknown or mistyped keyword, `provider=` included, returns `sc_agent_py.invalid_option` (agt-j-1 J1-12).
    - One send path: `Run::send` and `Run::batch` call one private sender, and in Python `run.send` is `run.send_batch` with one request (agt-j-1 J1-2), and `decidr` creates one Run per command. In Rust the Run is one struct whose single values are plain fields (NFR-SAG-0005); in Python the Run and Logger are `#[pyclass]` types, with no capsule or string-keyed option table crossing the boundary (agt-j-1 J1-1, J1-12).

15. **One owner (NFR-SAG-0004).** ULID minting, the provider table and its credential list, the model table and the session-log layout are owned by the Rust crate and exported by the wheel; the consumer template and every consumer call them and hold no copy.

### Rationale

Two consumers rebuilt the boundary and diverged, so the boundary moves into the wheel and integration code is generated from one template. A stored category and the send count replace message parsing and pinned id lists, and per-run limits and position-tagged results remove consumer workarounds; prerelease tooling leaves the product API so it does not freeze into the error contract.

Owner rulings (Rand Lee):

- 2026-11-20: category assignments not locked, "No hand built checks."
- 2026-11-21: time values as Duration types, discriminated-union results, no request-order requirement; records follow docs/plans/reduction-plan.md.
- 2026-11-24 (plan review, ADR-SAG-0003): amended in place; one owner per datum; providers as endpoints and models as rows; the run as a context object; one cache policy per call; one row per query sent, a link record per cache hit.
- 2026-11-24: fixed local model ids (`laya:en`; `decider-2b-v11` on port 18000 and `decider-4b-v2.1` on port 18001, both released 2026-09-24; `decider` resolves to 4b); configuration model rows; the merged table sorted newest first and verified by a unit test; no model names in consumer code (`sc_agent.model("haiku")` is a "red flag"); calls through `run.send` / `run.send_batch`; PyO3 calls never raise (`**rest` or `Option<&Bound>` arguments).
- Lead rulings 2026-11-24: single values as plain fields; result types inside the template; consent readers stay in consumers; `FakeRun` replaces consumers' Run doubles.

### Consequences

**Positive:**

- Every Python consumer renders the same integration module and holds only its prompts and result handling.
- No consumer parses error text or pins id lists, and all Jev data takes one session-log path.
- Prerelease tooling no longer ships in the product crate or wheel.

**Negative:**

- A breaking release for Rust dependents: the error type and call API change.
- The wheel moves to maturin's mixed layout.

### Alternatives Considered

#### Alternative 1: A separate `server` category for 500, 503 and 504

**Description:** Count these final replies as `server` instead of `busy`.

**Why Rejected:** It extends the fixed set for a distinction no consumer acts on; `busy` already means "resubmit later".

#### Alternative 2: A "Consumer integration" recipe in the README

**Description:** Document the integration for each consumer to copy.

**Why Rejected:** Hand copies already diverged.

#### Alternative 3: An import-time version check in the consumer module

**Description:** Raise on import when the generated header's versions differ from the installed sc-decidr or sitr.

**Why Rejected:** It hinders troubleshooting and development; mixed versions are documented as unsupported, not enforced.

### Implementation

The work lands on `rewrite/v1` and merges into `develop` in one PR; the tests are the acceptance criteria of REQ-SAG-0016, REQ-SAG-0017, REQ-SAG-0018, REQ-SAG-0702 and NFR-SAG-0003. Beyond the code: `CHANGELOG.md` (new) with "Breaking" and "Consumer migration" sections, the migration naming what judge-jev and plan-anchor delete (pinned id lists, `JEV.UNMAPPED`, message-text parsing, the `--timeout` refusal, the ignored `--max-workers`, the `provider="typesafe"` pin, result re-sorting, their retry loops and hand-written glue); the consumer template, `sc_agent/testing.py` and `sc_agent/__init__.pyi`; `floors.toml`; and the sc-decidr wheel, with the decidr and sitr APIs, on PyPI. Consumer-repository changes are out of scope.
