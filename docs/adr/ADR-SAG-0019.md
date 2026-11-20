# Decision: sc-decidr is the one Python decision harness: typed error fields, a stored error category, per-call limits and a generated consumer module

**Status:** Approved  
**Version:** 0.7.2  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-19  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-14  

---

## ADR-SAG-0019: sc-decidr is the one Python decision harness

**Status:** Approved  

### Related Documents

New with this ADR: REQ-SAG-0016 (typed error fields and the leaf field table), REQ-SAG-0017 (the category table and status refinement), REQ-SAG-0702 (per-call limits), REQ-SAG-0018 (the generated consumer module), NFR-SAG-0002 (0.1.0 detail JSON still reads) and NFR-SAG-0003 (one Python floor). REQ-SAG-0007 is amended so it covers decision values, which makes `retry_after_ms` a permitted transport hint. REQ-SAG-0601 and REQ-SAG-0602 are the sources of the status, send count and Retry-After. REQ-SAG-0701, REQ-SAG-0013 and REQ-SAG-0015 set the configured worker count, the worker cap and the timeout bounds. ADR-SAG-0004 sets the crate layering; this ADR extends its error chain with typed fields.

**Requirements:**

- REQ-SAG-0016
- REQ-SAG-0017
- REQ-SAG-0702
- REQ-SAG-0018
- NFR-SAG-0002
- NFR-SAG-0003
- REQ-SAG-0007
- REQ-SAG-0002
- REQ-SAG-0601
- REQ-SAG-0602
- REQ-SAG-0701
- REQ-SAG-0013
- REQ-SAG-0015

**Architecture Decisions:**

- ADR-SAG-0004

### Context

sc-decidr is the one harness through which sc-agent's Python consumers reach decision models: Jev, decider, Laya, Haiku and Luna. It owns model selection, API-key lookup, session logging (one record per query and response), the injectable general logger, resends and errors.

The owner requires that judge-jev, plan-anchor and every later Python consumer have identical integration for errors, API keys, model selection, the injectable general logger and session logging. They differ only in prompt authoring and in processing results and decision trees.

The two existing consumers each rebuilt the wheel boundary, and the copies diverged:

- **Errors.** judge-jev pins the original id list and falls back to `JEV.UNMAPPED`. plan-anchor matches message text (`"http 401"`).
- **Model.** judge-jev passes the model selector unchanged. plan-anchor pins `provider="typesafe"`, which disables Haiku and Luna.
- **Order.** Both re-sort `send_batch` results into input order.
- **Timeout.** judge-jev refuses `--timeout`. plan-anchor calls the unreleased `timeout_secs`.

Phase b already changed the error type, and this ADR builds on that base:

- `ErrorDetail.status: Option<u16>` and the provided method `ErrorId::status()` exist.
- `recovery` is `Option<Box<str>>`.
- `timeout_secs` exists on `send` and `send_batch`, validated as `RequestTimeout`.
- Python has the keyword arguments `keyring_service` and `keyring_account`.

Three gaps remain: most errors still lose their typed data; consumers have no category to branch on, so they pin id lists; and there is no per-call worker count or CLI timeout flag.

### Decision

sc-decidr makes the wheel the one consumer boundary for Python decision consumers, through these decisions. Decision numbers are stable; there is no decision 3.

1. **The wheel is the consumer boundary.**

   Every consumer concern except prompt authoring and result interpretation belongs in sc-decidr.

   | Concern | Provided by sc-decidr | Consumer does |
   | --- | --- | --- |
   | Model and provider | `model=` takes any selector string. `provider=None` ranks the detected keys (REQ-SAG-0103). | Pass the selector unchanged. Never set `provider`. |
   | API keys | The environment, dotenv and keyring, with `keyring_service`/`keyring_account` overrides. | Pass its keyring overrides, if any. |
   | Session log | One record per query and response, always written to the wheel's default location, tagged with `project` and `agent_role`. | Pass `project` and `agent_role`. The Rust session logger is not injectable from Python (REQ-SAG-0008). |
   | General log | The generated client obtains one Logger from the sc-decidr wheel, named after the client and writing `~/.sc-agent/logs/<client>.log.jsonl`, and injects it into every sc-decidr and sitr call (REQ-SAG-0022, ADR-SAG-0021). | Supply only the client name; the generated client passes the wheel-owned handle to both wheels. |
   | Errors | Typed fields with a stored `category`, `category()`, `retryable()` and `SagError` (decisions 2, 5 and 6). | Map a category to its own codes. Never parse message text. |
   | Limits | `timeout_secs`, `workers`, resends, the start budget and cancellation (decision 7). | Pass its own flags through. |
   | Batch order | `send_batch` returns input order (decision 8). | Nothing. |

   **The integration artifact is a generated module** (REQ-SAG-0018). The wheel ships an sc-compose template, `sc_agent/templates/consumer.py.j2`, as package data, and each consumer renders its integration module from it. To carry the file, the wheel moves to maturin's mixed layout; `import sc_agent` is unchanged.

   - The template covers the decision call, model pass-through, API keys, logging wiring, cancellation, `abort_on` and the error-to-category-to-code mapping. Prompt authoring and result processing stay in the consumer.
   - Its feature variables are `models` and `sitr` (`true` or `false`). Session logging and the injected general logger are not optional (REQ-SAG-0008, REQ-SAG-0022).
   - sc-compose is an external render tool, not a dependency of either wheel. The template's front matter declares its sc-compose minimum (`requires.cli`); the minimum is declared, not enforced.
   - The generated header records the sc-decidr version, and the sitr version when `sitr` is on, as troubleshooting information. Nothing checks it at runtime.
   - All Jev queries and responses go to the session log, so every consumer's Jev data takes the same path and can be filtered by repository and agent.

   **The lower-level API stays the contract.** `send`, `send_batch`, `category`, `retryable`, `SagError` and `CancelToken` remain public. The generated code uses only them and the standard library.

2. **Typed fields: `status` (exists), `attempts`, `retry_after_ms` and `category` (new).**

   `ErrorDetail` gains three optional fields beside phase b's `status`. Each is optional: absent on read means unset, and unset is not written. They are declared after `cause`. The struct keeps `Eq`.

   | Field | Type | Meaning |
   | --- | --- | --- |
   | `status` | `Option<u16>` | The HTTP status of the reply this error is about. |
   | `attempts` | `Option<u32>` | The sends made for this request, the first one included. |
   | `retry_after_ms` | `Option<u64>` | The last reply's valid `Retry-After`, in whole milliseconds. |
   | `category` | `Option<ErrorCategory>` | The error's category (decision 6). |

   `ErrorId` gains the provided methods `attempts()` and `retry_after_ms()`, each `None` by default, following `status()`.

   `detail()` remains the only builder. It stores the category and does the copy-up in one place:

   - A wrapper takes all four fields from its cause's detail.
   - A leaf keeps its own.

   The top-level fields are therefore those of the first leaf in the chain, so `SagError.status` and `SagError.category` always describe the same reply, and a wrapper needs no code of its own.

   REQ-SAG-0016 holds the leaf field table: which leaf variant sets which field.

   **`retry_after_ms`** is computed once, in `RetryAfter::parse`, from the integer seconds and nanoseconds the parser holds: rounded up to whole milliseconds, saturating at `u64::MAX`, never clamped. Fraction digits past the ninth are truncated by the parser. `ProviderReply` carries it beside the exact `retry_after_seconds`. REQ-SAG-0007 is amended to allow this rounding of a transport timing hint.

4. **Breaking minor release.**

   The Rust change breaks SemVer: fields are added to an exhaustive public struct, and the call-limits API changes (decision 7). Below 1.0 a minor bump is the breaking bump. For Python the change only adds things, except the batch return order (decision 8).

   All workspace crate and wheel versions move together (`[workspace.package].version` in `Cargo.toml`); `verify-version-lockstep` enforces this. sc-decidr and sitr are released together at the same version, both to PyPI. Mixed versions are not supported; this is documented, not enforced.

5. **Python errors.**

   `SagError` is one exception class importable from `sc_agent`. Its constructor takes `(id, message, detail)` and parses the detail JSON once. `args` stays `(id, message, detail_json)`, so copying and pickling rebuild it. Its read-only attributes are `id`, `message`, `detail`, `status`, `attempts`, `retry_after_ms`, `category` (`other` when the detail has none) and `retryable` (REQ-SAG-0016). This decision states behaviour, not mechanism, because pyo3 cannot subclass `PyException` in an abi3 wheel below Python 3.12, and the floor is 3.11.

   Result lines keep their shape; each level of `error` gains the new fields when they are set. A failed decision stays as data in its line; `SagError` is for command errors.

6. **The category is stored when the detail is built.**

   `sc-agent-types` gains `ErrorCategory`, an enum with `as_str()` and `retryable()`. Its values are `busy`, `quota`, `auth`, `network`, `response_invalid`, `validation`, `cancelled`, `config`, `log` and `other`.

   It also gains the macro `error_catalog!`. Each crate writes one table per error enum:

   ```rust
   error_catalog! { HttpError {
       ProviderBusy { .. } => ("http.provider_busy", Some(Busy)),
       Network { .. }      => ("http.network", Some(Network)),
       ...
   } }
   ```

   The macro expands to an exhaustive `catalog_row` match that returns the variant's id and its category, `None` for a wrapper. `ErrorId::id()` becomes `self.catalog_row().0`. A variant with no row does not compile.

   - Each crate keeps its own tables. No table is combined with another crate's, and no category is looked up across crates.
   - The two Python-binding ids become variants of a new `PyBindingError` enum in sc-agent-py, with its own table.
   - A leaf stores its row's category. A wrapper copies its cause's (decision 2).
   - `llm.api_status`, `decision.api_status` and `local.probe_status` store the status refinement of their status: 401 and 403 are `auth`, 402 is `quota`, 400 and 422 are `validation`, and 429, 500, 503 and 504 are `busy`.
   - 500, 503 and 504 are `busy` because the HTTP layer resends only 429, 502 and 529, so these arrive as final replies and must stay retryable.

   REQ-SAG-0017 holds the category table: the category of every id.

   Python reads the stored field through `sc_agent.category(detail)` and `sc_agent.retryable(detail)`. A detail with no stored category (written before categories existed, or foreign) is `other`.

   **`retryable` means "worth resubmitting as a new call later, after the wheel's own resends have run out."** It is true for `busy` and `network` only. A consumer that resubmits waits first, at least `retry_after_ms` when it is set, and does not wrap the wheel in an immediate retry loop.

7. **Per-call limits: `timeout_secs` (exists) and `workers` (new).**

   Both limits travel in `decidr::CallLimits { timeout: Option<RequestTimeout>, workers: Option<Workers> }`, which implements `Default`.

   - `send_with` and `send_batch_with` take `&CallLimits`. They replace phase b's unreleased `send_with_timeout` and `send_batch_with_timeout`. `SendOptions` is unchanged.
   - `Workers::new(i64)` checks 1..=64 and returns `decidr.invalid_workers`. `RequestTimeout::from_secs` checks 10..=3600 s. Validation is these two range checks only; out-of-range values are refused, never clamped.
   - Python: `send_batch(workers=, timeout_secs=)` and `send(timeout_secs=)`. CLI: `--workers N` (with `--batch` only) and `--timeout-secs SECS`.
   - A limit applies to its own call only. `None` means the configured value. A `workers` value below or above the configured count is honored.
   - `timeout_secs` replaces the per-send request timeout at both sites, `decision_transport` and `local_backend`. For a load-on-first-request runner the sends before the first 2xx get `load_timeout + timeout_secs`.
   - Every send gets the full timeout, resends included. The start budget limits when a send may start and never cuts a send in flight.

   REQ-SAG-0702 holds the requirements.

8. **`send_batch` returns its list in input order.**

   The returned list is sorted by `position`. `on_result` still receives each line as it finishes.

9. **Prerelease verification is real-provider integration tests; the integration gate is removed.**

   `decidr::integration_gate`, its re-exports `integration_cases` and `validate_integration_results`, the matching `sc_agent` functions and the surrounding report and gate tooling are removed before 0.2.0. `decidr.integration_evidence` is not an error id.

   Integration tests send real queries through the rendered consumer. One suite runs once per provider (TypeSafe, Anthropic, OpenAI); each run's environment holds only that provider's key, so the consumer's automatic selection resolves to it. A provider with no key is reported as not run; any other error fails. The developer runs the suite on a locally built wheel and fixes every failure before the prerelease is cut. The same suite then runs on the installed prerelease wheels on a clean install, and each run renders one XHTML panel (provider x platform) recording the prerelease tag and commit; the panels are committed and pushed. One passing platform is sufficient. REQ-SAG-0133 is superseded by this decision.

10. **Category assignments are part of the contract.**

    Moving an id to a different category, or changing the status refinement, is a breaking change (NFR-SAG-0002). Adding an id with a category is not.

11. **sitr is an optional, separate section of the consumer template.**

    sitr's Python API is in the separate sitr wheel (ADR-SAG-0005, ADR-SAG-0021). The template's sitr section is kept separate from the decision call: extraction runs outside the decision path, and its output is passed into the consumer's prompt authoring. The section uses the sitr wheel API. In this phase one offline test shows extraction output reaching a dry-run request; sitr is not used beyond that yet.

12. **One Python floor (NFR-SAG-0003).**

    - `floors.toml` at the repository root writes down the Python floor (`[python] floor`) and the sc-compose minimum (`[tools] sc-compose`). It only records values.
    - The pyo3 abi3 feature in `sc-agent-py` and `sc-treesitter` moves to the abi3 feature of the Python floor, so the wheel tag matches the `requires-python` the wheel declares.
    - Until sc-compose can read `floors.toml`, the template carries these values as literals, and one sc-agent unit test compares them with `floors.toml`.

### Rationale

Two consumers rebuilt the wheel boundary and the copies diverged, so every concern except prompt authoring and result interpretation moves into sc-decidr, and the integration code is generated from one template. Typed fields and a stored category replace message-text parsing and pinned id lists. Storing the category when the detail is built, and copying it up with the other fields, needs no lookup across crates and keeps `status` and `category` describing the same reply. Per-call limits and input-order batch results remove the consumer code that worked around their absence. Prerelease tooling leaves the product API so it does not freeze into the error contract.

### Consequences

**Positive:**

- Every Python consumer renders the same integration module and contains only its own prompts and result handling.
- No consumer parses error text or pins id lists.
- All Jev data from every consumer takes one session-log path.
- Prerelease tooling no longer ships in the product crate or wheel.

**Negative:**

- This is a breaking release for Rust dependents: `ErrorDetail` gains fields and `send_with_timeout` becomes `send_with`.
- `ErrorId` gains a required method, `base_category()`, so implementors outside the workspace must add it.
- `id()` moves into the macro across the error enums, a one-time mechanical diff.
- The order of the list `send_batch` returns changes.
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

#### Alternative 4: A catalog combined across crates, with the category looked up when a detail is read

**Description:**

Combine every crate's rows in `decidr::error_catalog()` and compute the category from the cause chain on read.

**Why Rejected:**

sc-agent-types cannot see higher crates' rows (ADR-SAG-0004), and a detail read back from JSON carries no rows. Storing the category at build time needs neither.

### Implementation

The work lands in sc-agent as fixes on top of the integration stack, with no mid-stack edits. The tests are the acceptance criteria of REQ-SAG-0016, REQ-SAG-0017, REQ-SAG-0018, REQ-SAG-0702, NFR-SAG-0002 and NFR-SAG-0003. A `/prerelease` is cut from the stack top for the consumers to build on.

Deliverables beyond the code:

- `CHANGELOG.md` (new), with "Breaking" and "Consumer migration" sections. The migration section names what judge-jev and plan-anchor delete: pinned id lists, `JEV.UNMAPPED`, message-text status parsing, the `--timeout` refusal, the ignored `--max-workers`, the `provider="typesafe"` pin, result re-sorting, their own retry loops and their hand-written integration glue.
- The consumer template and the `sc_agent.pyi` stub.
- `floors.toml`.
- sc-decidr and sitr released to PyPI at the same version.

Changes inside judge-jev, plan-anchor and other consumer repositories are out of scope.
