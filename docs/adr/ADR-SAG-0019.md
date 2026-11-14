# Decision: sc-decidr 0.2.0 is the one Python decision harness: typed error fields, an error catalog with chain categories, per-call limits and a generated consumer module

**Status:** Proposed  
**Version:** 0.5.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-14  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-14  

---

## ADR-SAG-0019: sc-decidr 0.2.0 is the one Python decision harness

**Status:** Proposed  

### Related Documents

REQ-SAG-0016 (typed error fields), REQ-SAG-0017 (the error catalog), REQ-SAG-0702 (per-call limits), REQ-SAG-0018 (the generated consumer module), NFR-SAG-0002 (0.1.0 detail JSON still reads) and NFR-SAG-0003 (one Python floor) are new with this ADR. REQ-SAG-0007 (no rounding of returned values) is amended here so it covers decision values, which makes `retry_after_ms` a permitted transport hint. REQ-SAG-0002 (errors carried from bottom to top) is unchanged; this ADR only adds detail. REQ-SAG-0601 and REQ-SAG-0602 (busy resend and quota) are the sources of the status, send count and Retry-After. REQ-SAG-0701 is the configured worker count; its default of 8 is unchanged. REQ-SAG-0013 and REQ-SAG-0015 set the worker cap of 64 and the local-runner timeouts with their 10..=3600 s bounds. ADR-SAG-0004 sets the crate layering and has a lower error reach a higher one as an id, message and cause chain; this ADR extends that chain with typed fields. CLI and Python parity (one keyword per `decidr send` flag) is ANC-006.

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

sc-decidr is the one harness through which sc-agent's Python consumers reach decision models: Jev, decider, Laya, Haiku and Luna. It owns model selection, API-key lookup, session logging (one JSONL record per query and response), the injectable general logger, resends and errors. Two consumers already exist, and more Python integrations are planned:

- judge-jev, a stack of more than 20 layers;
- plan-anchor, PR plan-anchor#90.

The owner requires that judge-jev, plan-anchor and every later Python consumer have identical integration for errors, API keys, model selection (the jev, haiku, luna, decider and laya alternates), the injectable general logger and session logging. They differ only in prompt authoring and in processing results and decision trees.

Comparing the two showed that each rebuilt the wheel boundary on its own, and that the two copies diverged:

- **Errors.**
  - judge-jev pins the 0.1.0 id list and maps by id and `status`, falling back to `JEV.UNMAPPED`.
  - plan-anchor matches message text (`"http 401"`) and reads a `recoverable` key that `ErrorDetail` does not have.
- **Model.**
  - judge-jev passes the user's model selector unchanged, so Haiku and Luna work.
  - plan-anchor pins `provider="typesafe"`, which disables them.
- **Order.** Both consumers re-sort `send_batch` results into input order.
- **Timeout.**
  - judge-jev refuses `--timeout`.
  - plan-anchor calls `timeout_secs`, which phase b added (agt-b-1.8, 05902db) and which is not yet released.

Phase b already changed the error type, and this ADR builds on that base. Those commits are on `integrate/phase-b` (merged by #86 at 745c1fb), not yet on `develop`; the implementation of this ADR is cut from `integrate/phase-b`.

- agt-zii (d827a04) added `ErrorDetail.status: Option<u16>` and a provided method `ErrorId::status()`, set for `http.provider_busy` and `decision.api_status`.
- It changed `recovery` to `Option<Box<str>>`.
- agt-b-1.8 added `timeout_secs` to `send` and `send_batch`, validated as `RequestTimeout` in the range 10..=3600 s with error `decidr.invalid_request_timeout`. It is applied at both timeout sites (`decision_transport` and `local_backend`).
- Python gained the keyword arguments `keyring_service` and `keyring_account`.

Three gaps remain:

1. The typed data is still lost for most errors, including busy, quota, network, too-large, 400, 401 and 422.
2. There is no catalog, so consumers pin id lists.
3. There is no per-call worker count, and the CLI has no timeout flag.

### Decision

sc-decidr 0.2.0 makes the wheel the one consumer boundary for Python decision consumers, through these twelve decisions:

1. **The wheel is the consumer boundary.**

   Every consumer concern except prompt authoring and result interpretation belongs in sc-decidr. Each concern is listed here with the surface that provides it.

   | Concern | Provided by sc-decidr | Consumer does |
   | --- | --- | --- |
   | Model and provider | `model=` takes any selector string: a short name (`jev`, `haiku`, `luna`), a model id or a local runner name. `provider=None` ranks the detected keys (REQ-SAG-0103). | Pass the user's selector unchanged. Never pin `provider` unless the user asked for it. |
   | API keys | The environment, dotenv and keyring, with `keyring_service`/`keyring_account` overrides. A missing key is a typed `auth` error. | Nothing. |
   | Session log | One JSONL record per query and response, at the default location, or sent to the `session_log` callable. | Nothing, unless it redirects the log. |
   | General log | The `general_log` callable. The wheel calls it and never owns the caller's logger. | Optionally inject one. |
   | Errors | Typed fields, `ERROR_IDS`, `category()`, `retryable()`, and `SagError` attributes (decisions 2 to 6). | Map a category to its own codes. Never parse message text. |
   | Limits | `timeout_secs`, `workers`, resends, the 90 s start budget and cancellation (decision 7). | Pass its own flags through. |
   | Batch order | `send_batch` returns its list in input order, while `on_result` still streams in finish order (decision 8). | Nothing. |

   **The integration artifact is a generated module, not a recipe** (REQ-SAG-0018). A README recipe is copied by hand, and two copies already diverged. Instead the wheel ships an sc-compose template, `sc_agent/templates/consumer.py.j2`, from which each consumer renders its integration module with sc-compose at or above the minimum the template declares in its front matter, in the skills' form (`requires.cli`, an `sc-compose` entry with `minimum_version`). The minimum is declared, not enforced. sc-compose is an external render tool, not a dependency of either wheel:

   - **What it covers.** The call; passing the model selector through unchanged, never pinning `provider`; API keys; general-logger and session-log wiring; and error → category → consumer code. The consumer passes in its own category-to-code map.
   - **What it leaves out.** Prompt authoring and result and decision-tree processing stay in the consumer.
   - **Versioning.** The template ships inside the sc-decidr wheel as package data, so its version is the installed wheel's by construction. The wheel moves to maturin's mixed layout (`python-source`, the native module re-exported by `sc_agent/__init__.py`, so `import sc_agent` is unchanged) to carry the file. sitr is a separate wheel, version-locked with sc-decidr: both are released together at the same version (decision 4), and the template is tested against both at that version. A rendered module consumes sc-decidr, plus sitr when the sitr section is on. Mixed sc-decidr and sitr versions are not supported; this is documented, not enforced. The generated file's header records the sc-decidr version, and the sitr version when the sitr section is on, as troubleshooting information only. Nothing checks it at runtime: a fail-fast check would only make troubleshooting and development harder.
   - **Feature variables.** Features are chosen only by template variables: `logging` (`general`, `session` or `both`), `models` and `sitr` (`true` or `false`). Rendering is sc-compose's responsibility; this ADR adds no rules about the sc-compose version or its output beyond the consumer CI check below.
   - **Consumer CI.** Consumer CI renders the file again and diffs it, so any hand edit fails.
   - **README.** `crates/sc-agent-py/README.md` keeps a short pointer to the template, with the render command and the CI check.

   **The lower-level API stays the contract.** `send`, `send_batch`, `category`, `retryable`, `ERROR_IDS` and `SagError` remain public and documented. The generated code uses only them and the standard library, and nothing is reachable only through generated code. A sc-agent test renders the template with every feature combination and runs each result against the loopback runner.

   The wheel also ships a `sc_agent.pyi` type stub, so consumers get signatures and field names without reading Rust.

2. **Typed fields: `status` (exists), `attempts` and `retry_after_ms` (new).**

   `ErrorDetail` keeps phase b's `status: Option<u16>` and gains two fields. All three are `#[serde(default, skip_serializing_if = "Option::is_none")]` and are declared after `cause`. The struct keeps `Eq`.

   | Field | Type | Meaning |
   | --- | --- | --- |
   | `status` | `Option<u16>` | The HTTP status of the reply this error is about. |
   | `attempts` | `Option<u32>` | The sends made for this request, the first one included. |
   | `retry_after_ms` | `Option<u64>` | The last reply's valid `Retry-After`, in whole milliseconds. |

   `ErrorId` follows the pattern phase b set with `status()`. It gains two more provided methods, each returning `None` by default:

   - `attempts(&self) -> Option<u32>`
   - `retry_after_ms(&self) -> Option<u64>`

   There is still no extension map: it would be untyped, would show no field names in Python or rustdoc, and would turn every new datum into a convention instead of a reviewed field.

   `detail()` remains the only builder and must not be overridden. It does the copy-up in one place, keyed by the catalog rule (decision 6), so that the top-level fields and the category always describe the same level:

   - A level whose rule is `inherit` (a wrapper) takes all three fields from its cause's detail and none of its own.
   - A level whose rule is `base` reports only its own accessors and does not fill from its cause.

   The top-level fields are therefore exactly the fields of the deciding level: the first level from the top whose rule is `base`. `category()` uses that same level (decision 6). The result:

   - every wrapper carries its cause's fields with no code of its own, including wrappers added later;
   - a leaf variant overrides only the methods for data it holds;
   - `SagError.status` and `SagError.category` always describe the same reply;
   - each crate's existing test, which checks that `detail()` equals the value built from the accessors, covers all three fields and the copy-up.

   Leaf variants and their fields (`—` means not set):

   | Id | status | attempts | retry_after_ms |
   | --- | --- | --- | --- |
   | `http.provider_busy` | `last.status` (exists) | `attempts` | `last.retry_after_ms` |
   | `http.quota_exhausted` | `reply.status` | `attempts` | `reply.retry_after_ms` |
   | `http.network` | — | `attempts` | — |
   | `http.response_too_large` | `status`, `None` when 0 (refused before the status was read) | — | — |
   | `llm.api_status` | `reply.status` | — | `reply.retry_after_ms` |
   | `decision.api_bad_request` | 400 | — | — |
   | `decision.api_validation` | 422 | — | — |
   | `decision.api_status` | `status` (exists) | — | — |
   | `decision.response_parse` | `status` | — | — |
   | `local.probe_status` | `status` | — | — |
   | `decision.cancelled` | — | `sends` | — |

   `decision.schema_mismatch` and `decision.backend` set no fields of their own. `decision.backend` is a wrapper (rule `inherit`) and takes all three fields from its cause through the copy-up. `decision.schema_mismatch` has its own category, `response_invalid`, so it reports only its own fields, all `None`; its reply was 2xx in any case.

   **Deriving `retry_after_ms`.** It is computed once, in `RetryAfter::parse`, from the integer seconds and nanoseconds the parser already holds, not from the `f64`:

   - It is the ceiling in milliseconds, so a caller that waits that long never retries early.
   - It saturates at `u64::MAX`, because 32 digits are valid input.
   - It is not clamped. The 0–30 s clamp applies only to the resend holdoff.
   - `0` gives `Some(0)`, and a malformed header gives `None`.

   `ProviderReply` carries it as `retry_after_ms: Option<u64>` next to the exact `retry_after_seconds: f64`, which is unchanged. The fields read that value and never recompute it.

   **REQ-SAG-0007.** The owner ruled on 2026-11-14 that the rule protects decision-model values (Jev probabilities, confidences and values computed from them). This PR amends REQ-SAG-0007 to say so, and to allow transport timing hints to be rounded as their owning ADR states. The exact value stays available in Rust and in the message text.

   **Not typed in 0.2.0:** `NetworkKind` (timeout, connect, tls, other). The category is `network` whatever the kind. The kind can be added as a later field without breaking anything (decision 3).

3. **`#[non_exhaustive]` on `ErrorDetail`.**

   `ErrorDetail` becomes `#[non_exhaustive]`, and its fields stay `pub` so they can still be read. Phase b already broke struct literals by adding `status`. With `#[non_exhaustive]`, this release makes the last field addition that breaks anything.

   Outside `sc-agent-types`, a value is built with:

   - `new`
   - `with_recovery`
   - `with_status`, which exists
   - `with_attempts` (new)
   - `with_retry_after_ms` (new)
   - `foreign`
   - `map_text` (new)

   `map_text(self, f: &impl Fn(String) -> String)` rewrites `id` and `message` throughout the chain and keeps `recovery` and every field. It replaces the struct literal in `sc-agent-http`'s production redaction code (`map_detail`), which `#[non_exhaustive]` would make illegal. A redaction test asserts that all three fields survive it.

4. **Version 0.2.0.**

   The Rust change breaks SemVer:

   - fields are added to an exhaustive public struct;
   - `#[non_exhaustive]` is added;
   - `recovery` changes type (phase b);
   - the call-limits API changes (decision 7).

   The crates are published on crates.io (`release/publish-artifacts.toml`). Below 1.0 a minor bump is the breaking bump, so `^0.1` dependents are not moved silently, and a 0.1.1 would break them. For Python the change only adds things, except the batch return order (decision 8), which the CHANGELOG states.

   Everything on the sc-agent release version moves from 0.1.5 to 0.2.0: every workspace crate, sc-treesitter and sitr included, and both wheels. sc-decidr and sitr are version-locked: they are released together at the same version, so one version gives one tag and one pin. Mixed versions are not supported, and this is documented, not enforced. The files to bump are:

   - `Cargo.toml` (`[workspace.package]`);
   - `crates/sc-agent-py/Cargo.toml` and `pyproject.toml`, and `crates/sc-treesitter-py/Cargo.toml` and `pyproject.toml`, whose literal 0.1.5 becomes 0.2.0;
   - `Cargo.lock`;
   - `fixtures/cli/decidr-cli-model.json` and the generated CLI docs (`scripts/generate-cli-docs.sh`), which also change because of the new flags.

   Publishing follows `publish_order`.

5. **Python errors.**

   `SagError` becomes a real exception class, `#[pyclass(extends = PyException)]`. Its `__init__(id, message, detail)` parses the detail JSON once:

   - `args` stays `(id, message, detail_json)`, so copying and pickling rebuild it.
   - A `SagError` built in Python (`SagError("x", "y", "{}")`) has every attribute, set to `None` or the default.
   - Both construction sites (`sag_error`, `worker_panicked`) and `invalid_option` go through it.

   Its read-only attributes:

   | Attribute | Value |
   | --- | --- |
   | `id`, `message`, `detail` | The three args. |
   | `status`, `attempts`, `retry_after_ms` | From the top-level detail, which carries its cause's fields through the copy-up. |
   | `category` | `category(detail)` (decision 6). |
   | `retryable` | `True` for categories `busy` and `network`. |

   Result lines keep their shape. Each level of `error` gains the new fields when they are set. A failed decision stays as data in its line; `SagError` is for command errors.

6. **The catalog and categories, generated from the enums and computed over the chain.**

   **Rust.** `sc-agent-types` gains:

   - `ErrorCategory`, a `#[non_exhaustive]` enum with `as_str` and `retryable()`. Its values are `busy`, `quota`, `auth`, `network`, `response_invalid`, `validation`, `cancelled`, `config`, `log` and `other`.
   - `CategoryRule`, either `Base(ErrorCategory)` or `Inherit` (for wrappers).
   - The macro `error_catalog!`. Each crate writes one table per enum, and each row carries the variant's shape:

   ```rust
   error_catalog! { HttpError {
       ProviderBusy { .. } => ("http.provider_busy", Base(Busy)),
       Network { .. }      => ("http.network", Base(Network)),
       ...
   } }
   ```

   The macro expands to two items:

   - an inherent `const fn catalog_row(&self) -> (&'static str, CategoryRule)`, holding the exhaustive match. Inside its own crate, `#[non_exhaustive]` does not relax exhaustiveness, so a variant with no row fails to compile.
   - `impl ErrorCatalog for E { const ROWS: &'static [(&'static str, CategoryRule)] }`.

   `ErrorId::id()` becomes `self.catalog_row().0` in each impl.

   The two Python-binding ids, `sc_agent_py.invalid_option` and `sc_agent_py.worker_panicked`, become variants of a small `PyBindingError` enum in sc-agent-py with its own table, so every row comes from an enum. `decidr::error_catalog()` concatenates every decidr-reachable crate's `ROWS`; sc-treesitter's ids are not decidr errors and are excluded.

   **Category over the chain.** `ErrorDetail::category(&self) -> ErrorCategory` works on a detail, not a bare id:

   1. Walk from the top through levels whose rule is `Inherit` to the deciding level: the first level with a `Base` rule. This is the level whose fields the top carries (decision 2).
   2. Apply the status refinement at that level, using that level's `status`.
   3. Return the result. A foreign id before any `Base` level, or a chain with no catalog id, gives `other`.

   The status refinement applies only to `llm.api_status`, `decision.api_status` and `local.probe_status`, whose base category is `other`:

   | Status | Category |
   | --- | --- |
   | 401, 403 | `auth` |
   | 402 | `quota` |
   | 400, 422 | `validation` |
   | 429 | `busy` (in practice only `local.probe_status`; the HTTP layer resends 429 everywhere else) |
   | 500, 503, 504 | `busy`: a transient server-side failure. The HTTP layer resends only 429, 502 and 529, so these arrive as final replies and must stay retryable. judge-jev treated every 5xx as transient. |
   | anything else | the base category (`other`) |

   Rejected: a separate `server` category. It would extend the owner's fixed category set for a distinction no consumer has asked to act on; `busy` already means "the provider could not serve this now; resubmit later".

   The rule lives in Rust only. Python exposes the same function:

   - `sc_agent.category(detail_or_id, status=None) -> str`, which takes a detail dict, a detail JSON string or a bare id;
   - `sc_agent.retryable(detail_or_id) -> bool`;
   - `sc_agent.ERROR_IDS: frozenset[str]`.

   **`retryable` means "worth resubmitting as a new call later, after the wheel's own resends have run out."** The wheel has already applied `max_sends`, the 90 s start budget and the provider pause before returning a `busy` or `network` failure. A consumer that resubmits waits first, at least `retry_after_ms` when it is set, and must not wrap the wheel in an immediate retry loop. `retryable` is true for `busy` and `network` only.

   An id outside the catalog gives `other` and does not raise. Foreign cause ids (`io.*`, `foreign.source`) and newer ids seen by an older wheel are therefore safe. `id in ERROR_IDS` is the membership test.

   **Base categories** (the `error_catalog!` rows). `inherit` is a wrapper whose category is its cause's.

   | Category | Ids |
   | --- | --- |
   | busy | `http.provider_busy` |
   | quota | `http.quota_exhausted` |
   | auth | `decidr.no_provider_key`, `secrets.key_missing`, `secrets.key_malformed`, `secrets.keyring_read`, `secrets.keyring_timeout`, `secrets.keyring_construction_refused` |
   | network | `http.network`, `local.load_timeout` |
   | response_invalid | `decision.response_parse`, `decision.schema_mismatch`, `http.response_too_large`, `llm.reply_invalid`, `llm.no_text`, `llm.truncated`, `llm.output_not_json`, `decision_llm.answer_ids`, `decision_llm.answer_invalid`, `local.probe_parse` |
   | validation | `decision.request_parse`, `decision.duplicate_key`, `decision.image_parts`, `decision.limits`, `decision.api_bad_request`, `decision.api_validation`, `decision_llm.invalid_max_tokens`, `decidr.usage`, `decidr.request_invalid`, `decidr.limit_exceeded`, `decidr.input_not_found`, `decidr.image_format`, `decidr.invalid_request_timeout`, `decidr.invalid_workers` (new), `http.invalid_request`, `log.invalid_slug`, `types.invalid_model_id`, `types.unknown_short_name`, `types.unknown_provider`, `types.invalid_var_name`, `types.invalid_session_id`, `sc_agent_py.invalid_option` |
   | cancelled | `decision.cancelled`, `decidr.signal` |
   | config | `config.*` (all eight), `decidr.unknown_runner`, `local.model_not_served`, `http.invalid_endpoint`, `http.key_on_plain_http`, `secrets.invalid_location`, `secrets.dotenv_read`, `types.invalid_release_date`, `types.model_on_two_lines`, `runner.invalid_config` |
   | log | `log.*` except `log.invalid_slug` |
   | other | `decision.api_status`, `llm.api_status`, `local.probe_status` (status-refined), `llm.refused`, `decision.encode`, `decision_llm.encode`, `decision_llm.invariant`, `decision.shutdown`, `decidr.encode`, `decidr.write`, `decidr.input_read`, `decidr.image_read`, `sc_agent_py.worker_panicked` |
   | inherit | `decision.http`, `decision.backend`, `llm.http`, `decision_llm.llm`, `local.http`, `local.decision`, `runner.session_log`, `decidr.config`, `decidr.types`, `decidr.secrets`, `decidr.log`, `decidr.local`, `decidr.decision`, `decidr.http`, `decidr.llm`, `decidr.runner` |

   The implementing sprint adds any id that lands on the b stack after this ADR, with a category, in the same table. A missing row fails to compile, so none can be forgotten.

7. **Per-call limits: `timeout_secs` (exists) and `workers` (new).**

   Both limits travel together in `decidr::CallLimits { timeout: Option<RequestTimeout>, workers: Option<Workers> }`, which is `#[non_exhaustive]` and implements `Default`:

   - `send_with` and `send_batch_with` take `&CallLimits`. They replace phase b's unreleased `send_with_timeout` and `send_batch_with_timeout`.
   - `SendOptions` is unchanged, so its literals keep compiling.
   - `Workers::new(i64)` checks the range 1..=64 (`MAX_WORKERS`) and returns `decidr.invalid_workers`.
   - `RequestTimeout::from_secs` is unchanged (phase b).

   There is one validator per value, used by Python and the CLI alike. `RunnerConfig::new` never receives an invalid count, so `runner.invalid_config` remains purely a config-load error.

   | Surface | `workers` | `timeout_secs` |
   | --- | --- | --- |
   | `sc_agent.send_batch` | `workers: int \| None = None` (new) | `timeout_secs: float \| None = None` (exists) |
   | `sc_agent.send` | not accepted: an unknown keyword is a `TypeError`, since one request uses one worker | exists |
   | `decidr send` | `--workers N` (new; refused without `--batch`, with `decidr.invalid_workers`) | `--timeout-secs SECS` (new) |

   - **Scope.** A limit applies to its own call only. It never writes the configuration and never affects another call. `None` means the configured value.
   - **Python argument handling.** Arguments are taken as `Bound<PyAny>` and extracted by hand, so every bad value of an accepted keyword raises `SagError`, never `TypeError` or `OverflowError`. An unknown keyword, such as `workers` on `send`, raises Python's own `TypeError`:
     - `bool` is refused for both arguments;
     - `timeout_secs` accepts an `int` or a `float`;
     - `workers` accepts an `int` only.
   - **`workers`.**
     - A value below the configured `workers` is honored, and so is any value above it up to 64. The configured value is a default (REQ-SAG-0701), not a floor or a cap.
     - The queue capacity follows the effective count.
     - The effective count is the `workers` field of the runner's existing `runner.stream.start` general-log event.
   - **`timeout_secs`.** This is phase b's contract, recorded here:
     - The range is 10..=3600 s, the same as a local runner's `request_timeout_secs`.
     - It replaces 120 s, or the runner's configured timeout, as the per-send request timeout (`Timeouts::request`), at both sites:
       - `decision_transport`, for every send to a cloud provider;
       - `local_backend`, where for a load-on-first-request runner (Laya) the sends before the first 2xx get the deadline `load_timeout + timeout_secs`.
     - The 10 s connect timeout is unchanged.
   - **Injected transport.** A context with an injected transport has no request timeout to replace, so `timeout_secs` with an injected transport is refused with `decidr.invalid_request_timeout`. Ignoring it silently would break the no-silent-no-op rule.
   - **Interaction with resends.**
     - Every send gets the full timeout, resends included.
     - A send that times out is `http.network`, and it is resent under `max_sends` and the 90 s start budget (REQ-SAG-0601).
     - The budget limits when a send may start; it never cuts a send that is in flight. A send therefore starts less than 90 s after the first send began, and in the worst case ends at that start plus `timeout_secs`. Time spent queued or host-paused before the first send is not counted.
     - A `timeout_secs` above 90 s is valid. It makes a send that times out the last one.
   - **No clamping.** Out-of-range values are refused, never clamped.

8. **`send_batch` returns its list in input order.**

   The returned list is sorted by `position`. `on_result` still receives each line as it finishes. Both existing consumers re-sort the list themselves, so the wheel does it once.

9. **Prerelease integration tooling leaves the product API.**

   `decidr::integration_gate` (about 400 lines that validate prerelease integration results) is public decidr API today, re-exported as `integration_cases` and `validate_integration_results`, and exposed in the wheel as `sc_agent.integration_cases` and `sc_agent.validate_integration_results`. Its error, `decidr.integration_evidence`, would otherwise enter the error catalog and become a permanent part of the product's error contract.

   Before 0.2.0 freezes the catalog, the module and both Python functions move out of the published crate and wheel into release tooling that is not published (for example a `publish = false` workspace crate or script used by `/prerelease`). `decidr.integration_evidence` is not a catalog id. The prerelease flow keeps working; only its home changes.

10. **Category assignments are part of the contract.**

    Consumers map categories to their own codes, so moving an id to a different category, or changing the status refinement, is a breaking change and needs a breaking version bump (a minor bump below 1.0). Adding an id with a category is not breaking. NFR-SAG-0002 records this.

11. **sitr is an optional, separate section of the consumer template.**

    sitr is published on PyPI (package `sitr`, module `sc_treesitter`) and on crates.io (`sitr`), first as 0.1.0, with wheels for the same six platforms as sc-decidr. It is pinned to the same sc-agent release as sc-decidr (decision 4).

    The template's sitr section is kept separate from the decidr call: tree-sitter extraction runs outside the decision path, and its output is passed into the consumer's prompt authoring. In this phase the section only imports `sc_treesitter` lazily, inside the section, and one offline test shows extraction output from a checked-in source file reaching a dry-run request. It is not used beyond that yet.

12. **One Python floor (NFR-SAG-0003).**

    - The Python floor is the minimum declared by `[project].requires-python` in `crates/sc-agent-py/pyproject.toml`; every other place that repeats it is checked against it in CI.
    - The pyo3 abi3 feature in `sc-agent-py`, `sc-treesitter-py` and `sc-treesitter` (its optional pyo3) moves from `abi3-py310` to the abi3 feature of the Python floor, so the wheel tag matches the `requires-python` both wheels already declare.
    - CI builds both wheels and runs their pytest suites on the Python floor; the `sc-agent-py` and `treesitter` jobs on `integrate/phase-b` already do.
    - The generated consumer module's script header declares the Python floor as its `requires-python`.

### Rationale

Two consumers rebuilt the wheel boundary on their own and the copies diverged (Context), so every concern except prompt authoring and result interpretation moves into sc-decidr (decision 1), and the integration code is generated from one template rather than copied from a recipe. Typed fields and a catalog with chain categories replace message-text parsing and pinned id lists (decisions 2 and 6). Copy-up keyed by the catalog rule makes the top-level fields and the category describe the same level, so `status` and `category` never disagree. Per-call limits and input-order batch results remove the consumer code that worked around their absence (decisions 7 and 8). Prerelease tooling leaves the product API so it does not freeze into the error contract (decision 9). The changes break SemVer for Rust dependents, so the version is 0.2.0 (decision 4).

### Consequences

**Positive:**

- Prerelease tooling no longer ships in the product crate or wheel.
- Every Python consumer renders the same integration module from the installed wheel and contains only its own prompts and result handling; a hand edit fails the consumer's CI.
- No consumer parses error text or pins id lists.
- A missing catalog row is a compile error, and copy-up needs no per-wrapper code.
- `#[non_exhaustive]` on `ErrorDetail`, `ErrorCategory` and `CallLimits` makes future additions non-breaking.

**Negative:**

- This is a breaking 0.2.0 for Rust dependents:
  - `ErrorDetail` literals become builder calls;
  - `recovery` changes type;
  - `send_with_timeout` becomes `send_with`.
- `id()` moves into the macro across 12 enums, which is a one-time mechanical diff.
- The order of the list `send_batch` returns changes, as the CHANGELOG states.
- The wheel moves to maturin's mixed layout to carry the template; `import sc_agent` is unchanged.
- Python 3.10 users lose the wheel; `requires-python` already excluded them.


### Alternatives Considered

#### Alternative 1: A separate `server` category for 500, 503 and 504

**Description:**

Add a `server` category instead of counting these final replies as `busy`.

**Why Rejected:**

It would extend the owner's fixed category set for a distinction no consumer has asked to act on; `busy` already means "the provider could not serve this now; resubmit later".

#### Alternative 2: A "Consumer integration" recipe in the README

**Description:**

Document the integration as a short recipe in `crates/sc-agent-py/README.md` for each consumer to copy.

**Why Rejected:**

A recipe is copied by hand, and two hand-written copies already diverged (Context).

#### Alternative 3: An import-time version check in the consumer module

**Description:**

Raise on import when the versions recorded in the generated header differ from the installed sc-decidr or sitr.

**Why Rejected:**

Owner direction: a fail-fast check only makes troubleshooting and development harder. Mixed versions are documented as unsupported, not enforced.

### Implementation

Tests, written with the implementation:

- **Fields.** For every leaf row in decision 2:
  - a Rust test builds the error and asserts its `detail()` fields;
  - a Python test reads the same values through `SagError` or the result line, end to end, using the loopback runner where an HTTP reply is needed.
- **Copy-up.**
  - `decision.http` over `ProviderBusy` carries the busy fields at the top level.
  - `decision.backend` over a 401 `llm.api_status` carries `status == 401`.
  - `map_text` keeps all three fields.
- **`retry_after_ms`.** These values from `RetryAfter::parse`:

  | Retry-After | `retry_after_ms` |
  | --- | --- |
  | `1.1` | 1100 |
  | `2.0001` | 2001 |
  | `0` | `Some(0)` |
  | `0.0000001` | 1 |
  | 32 nines | `u64::MAX` |
  | `abc` | `None` |
- **Catalog.**
  - A `compile_fail` doctest in sc-agent-types shows that a variant with no row does not compile.
  - Each crate's `every()` sample values map to ids that are in `ROWS`.
  - `error_catalog()` has no duplicate ids.
  - The Python test that `ERROR_IDS` equals the catalog is plumbing only.
- **Category.**
  - `decision.http` over `ProviderBusy` is `busy`.
  - Over `QuotaExhausted` it is `quota`, and over `Network` it is `network`.
  - `decision.backend` over a 401 `llm.api_status` is `auth`.
  - `local.probe_status` with 429 is `busy`.
  - `decision.api_status` with 500, 503 or 504 is `busy`, and with 501 is `other`.
  - A detail whose top-level `status` comes from level N gets its `category` from level N.
  - `io.ConnectionRefused` is `other`.
- **`SagError`.**
  - `SagError("x", "y", "{}")` has `status` `None` and `category` `"other"`.
  - A pickle round trip keeps `status`.
- **`workers`.**
  - A Rust test shows the `RunnerConfig` built from `CallLimits` has the requested count.
  - A Python test with `workers=2` reads `workers == 2` from the `runner.stream.start` event received by its `general_log` callable.
  - `0`, `65`, `-1`, `2.5`, `"2"` and `True` each raise `SagError` with `decidr.invalid_workers` before any request is sent.
  - The CLI refuses `--workers` without `--batch`.
  - None of these use sleeps or timing assertions.
- **`timeout_secs`.**
  - The loopback runner in `hang` mode with `max_sends = 1` and `timeout_secs=10` returns a line with:
    - top id `decision.http`;
    - cause `http.network` with `attempts == 1`;
    - a message beginning `timeout failure`.

    The server never answers, so the outcome does not depend on timing.
  - The existing phase b tests cover the bounds and the two sites. New tests cover:
    - the refusal with an injected transport;
    - the CLI flag;
    - a ManualClock Rust test with `timeout_secs` above 90 s showing the send is not cut.
- **Isolation.** A call with overrides followed by a call without them shows the second uses the configured values.
- **Order.** `send_batch` returns its list in input order while `on_result` sees finish order.
- **Consumer template (REQ-SAG-0018).**
  - Every feature combination renders and runs against the loopback runner: success, error-to-code mapping, session-log records and general-logger events.
  - The rendered header records the installed sc-decidr version, and the sitr version when `sitr` is true.
  - The sitr offline test: extraction from a checked-in source file reaches a dry-run request; with `sitr` false the module imports without `sc_treesitter`.
  - The rendered modules reference only the public API and the standard library.
- **Python floor (NFR-SAG-0003).** Both built wheels carry the abi3 tag of the Python floor, and the CI check fails when a repeated floor differs from it.
- **Compatibility (NFR-SAG-0002).** A checked-in 0.1.0 detail JSON with a two-level chain:
  - deserializes with every new field `None`;
  - serializes back to the original bytes.

Delivery:

The work lands in sc-agent on the `integrate/phase-b` line, as fixes on top with no mid-stack edits, in phase b sprints the planner cuts from this ADR. A `/prerelease` is cut from the stack top for the consumers to validate. This includes the consumer template and its tests, the abi3 and CI changes, and the CHANGELOG.

The 0.2.0 release MUST publish both sc-decidr and sitr to PyPI at the same version, not only as GitHub prerelease assets. Today PyPI has only 0.1.0; 0.1.2 to 0.1.5 exist only on GitHub, and consumers can pin only what is on PyPI.

Deliverables beyond the code:

- `CHANGELOG.md` (new), with "Breaking" and "Consumer migration" sections. The migration section names what judge-jev and plan-anchor can delete:
  - the pinned id lists;
  - `JEV.UNMAPPED`;
  - message-text status parsing;
  - the `recoverable` lookup;
  - the `--timeout` refusal and the ignored `--max-workers`;
  - the `provider="typesafe"` pin;
  - the result re-sorting;
  - their hand-written integration glue, replaced by the rendered consumer module, with the render command and the CI regenerate-and-diff check.
- The consumer template, `sc_agent/templates/consumer.py.j2`, and the README pointer to it.
- `sc_agent.pyi`.
- The regenerated CLI model and docs.
- The version files listed in decision 4.
- sc-decidr 0.2.0 and sitr 0.2.0 on PyPI.

Changes inside judge-jev, plan-anchor and other consumer repositories are out of scope, including plan-anchor dropping its Python 3.9 CI floor.
