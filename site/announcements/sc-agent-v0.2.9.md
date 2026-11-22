# sc-agent v0.2.9 — Typed Failures, Per-Call Limits and One Generated Python Client

**Released:** November 18, 2026 · **Install:** `brew install randlee/tap/decidr` (Homebrew — installs `decidr` and `sitr`), `pip install sc-decidr==0.2.9` (the `sc_agent` Python module), `pip install sitr==0.2.9` (the `sc_treesitter` module), `cargo install decidr` (crates.io), or the platform archives on the [release page](https://github.com/randlee/sc-agent/releases/tag/v0.2.9)

[README](https://github.com/randlee/sc-agent/blob/main/README.md) · [Requirements](https://github.com/randlee/sc-agent/tree/main/docs/requirements) · [ADRs](https://github.com/randlee/sc-agent/tree/main/docs/adr) · [Release notes](https://github.com/randlee/sc-agent/releases/tag/v0.2.9)

This is the first stable release of the 0.2 line, and it aggregates five days and
seventeen pre-releases of work since v0.1.0. Phase A made every backend reachable through
one request form. Phase B makes the *answer to a failure* the same shape too: an error
is no longer English text that a caller has to pattern-match, it is a typed detail with
a stored category, so a retry policy, a backoff and an abort decision are data
comparisons. The Python side stops being a thin mirror of the CLI and becomes the
harness ADR-SAG-0019 describes — typed error attributes, per-call limits, and one
consumer module generated from a template that ships inside the wheel.

Nothing about the launch interface changed. `decidr send`, `sc_agent.send`,
`sc_agent.send_batch`, `models` and `doctor` take the same arguments, against the same
backends, with the same exit codes. What changed is what comes back when something goes
wrong, and how a Python consumer is wired.

---

## Pipeline Orchestrator — failures you can branch on, and limits you can set per call

**As a pipeline orchestrator, I want every failure to carry its category, HTTP status, attempt count and Retry-After, so that retry, backoff and abort decisions are computed from fields instead of string matching.**

Every error detail now carries four typed fields — `status`, `attempts`, `retry_after_ms`
and `category` — plus `retryable`, derived from the leaf variant that produced the
error rather than from its message. The categories are fixed: `busy`, `quota`, `auth`,
`network`, `response_invalid`, `validation`, `cancelled`, `config`, `log`, `other`. A
wrapper copies its cause's fields up to the top level, so an `http.provider_busy`
surfaced as `decision.http` still reports `category: busy` and its `Retry-After`
without the caller unwrapping anything. Three status-refined ids (`llm.api_status`,
`decision.api_status`, `local.probe_status`) map HTTP codes to categories: 401/403 to
`auth`, 402 to `quota`, 400/422 to `validation`, 429/500/503/504 to `busy`.

The point is that a gating loop no longer has to guess. A `busy` with
`retry_after_ms` waits; a `quota` stops the batch; an `auth` fails loudly at once. If
you are on 0.1.x and your orchestrator matched on message text to decide any of that,
that logic can be deleted.

Limits are now per call, not per configuration. `sc_agent.send(..., timeout_secs=)` and
`decidr send --timeout-secs` set the send deadline for that call (10–3600 s, replacing
the 120 s default or a local runner's own timeout); `sc_agent.send_batch(..., workers=)`
and `decidr send --batch --workers` set the worker count for that batch (1–64), honoured
in either direction against the configured count. A bad value is refused before any
request leaves the process, as `decidr.invalid_request_timeout` or
`decidr.invalid_workers` — and `--workers` without `--batch` is refused with the same
id. A slow expensive model and a fast cheap one can share one configured fleet without
either one's timeout leaking into the other's call.

Batch results are ordered by contract: the returned list is in input order, while
`on_result` receives each line as it finishes. That is what makes incremental progress
reporting possible without reindexing, and what makes a partial batch traceable when
the run is cancelled mid-flight.

## Agent-Run Author — the same interface, now with a generated client and typed answers

**As an agent-run author, I want one generated consumer module that already knows the error contract, so that wiring a new pipeline to sc-agent is a render and an import instead of a copied helper.**

The `sc-decidr` wheel now ships `sc_agent/templates/consumer.py.j2`, a client derived
from the working plan-anchor consumer at `fff69b2`. Render it once per consumer with
`sc-compose` (>=1.8.0) and Python >=3.11, and you get a module that stays importable
without the wheel — `sc_agent` is imported only when a decision is actually made — and
that wraps `send` / `send_batch` with three things you would otherwise write yourself:
a required `abort_on` set, a `category_codes` map, and a `CancelToken` you own.

`abort_on` is validated to one of the empty set, `{"auth"}` or `{"auth", "quota"}` and
cancels the rest of a batch on the first result whose category is in the set. Category
classification is delegated to the wheel (`sc_agent.category(detail)`), never inferred
from message text, so a classification fix ships in a wheel upgrade rather than in every
consumer. Returned tuples keep the wheel's input order; `on_result` fires in finish
order. Prompt authoring and result interpretation deliberately stay in the consumer —
there is no hidden retry loop in the generated module.

Typed answers survive the same path: Jev batch records moved into the shared client
template, typed answers are normalised and validated against their question, and batch
records are redacted before serialisation. That work is covered end-to-end with the real
wheel, not a stub, because a consumer that passes stub tests and fails against the
published artifact is worse than no test.

## Python Integrator — typed errors, a real stub, and one floor

**As a Python integrator, I want the error surface typed and the package properly marked, so that my type checker and my `except` clauses agree with what the library actually raises.**

`SagError` now exposes `id`, `message`, `detail`, `status`, `attempts`,
`retry_after_ms`, `category` and `retryable` as properties, and the properties are
declared in `sc_agent/__init__.pyi` — a real stub shipped in the wheel, with the PEP 561
marker, so `mypy` and `pyright` see the same contract the interpreter enforces.
`category(detail)` and `retryable(detail)` are importable functions for when you hold a
detail JSON string rather than an exception, which is the normal case for a failed
result line. `SagError` renders as its id and message rather than a raw class
representation, and its fields are parsed once and cached rather than re-parsed per
attribute access.

Two long-standing integration annoyances are gone. `sc_agent` detaches its log
callbacks before interpreter finalisation, so a process that exits with a decision
in flight no longer dies in a teardown traceback. And the Python floor is one floor:
3.11. 0.1.x's split floors are collapsed into a single requirement that the wheel, the
stub and the distribution metadata all agree on (NFR-SAG-0003). The wheel also now
carries its README metadata and the consumer template as package data, so
`importlib.resources` finds the template on an installed wheel rather than only in a
source checkout.

## Ops Engineer — diagnosis by category, and a release that cannot ship blind

**As an ops engineer, I want a key or configuration fault to name its category and its trigger, so that a broken host is diagnosed from the error rather than from an experiment.**

The category table is now enforced per crate: every error id in every catalog table has
a category or a wrapper designation, and each catalog is tested against its enum's
variants — an id added without a category fails the build instead of surfacing as
`other` in production. Key handling got the same treatment. An ambiguous keyring
override (two sources naming different providers) is refused *before* lookup with
`secrets.keyring_construction_refused` rather than resolved by accident, rejected key
values are omitted from error text instead of echoed, and attached keys stay redacted in
native decision errors, not just in the Python path. Working-directory lookup failures
now classify as `config` with their cause preserved.

Release confidence is now mechanical. The publication gate is fail-closed: a prerelease
is cut only after a real-provider integration run, and release notes are carried into
the tagged version with empty notes rejected — so a tag cannot exist without a changelog.
Those integration runs are published as pages under [`site/reports/integration`](https://github.com/randlee/sc-agent/tree/main/site/reports/integration),
one per platform and provider (macOS and Colima, across TypeSafe, Anthropic and OpenAI),
alongside a reports index. When a provider changes its API, the evidence of what broke,
where, and on which platform is a URL.

## Static-analysis Tool Builder — `sitr` as an optional section of the consumer

**As a tool builder, I want tree-sitter extraction available inside the same generated client, so that a structural check and the decision call live in one module.**

`sitr` and `sc_treesitter` are unchanged in behaviour in this range — no new query or
grammar semantics — but the consumer template can now include them: render with
`sitr=true` and the generated module gains
`extract(language, query, source, captures=None)`, which imports the version-matched
`sitr` wheel only when called and delegates to `sc_treesitter.run_query`. Extraction
stays off the decision path; the captures feed the request you author, and the same
`call`/`call_batch` functions make the decision. Consumers that never render it pay
nothing.

---

## Upgrade Notes

- **Rust API break.** Error details are boxed in `DecisionError::Backend.detail`,
  `DecisionError::SchemaMismatch.detail`, `LogError::InvalidName.cause`,
  `LogError::RowsLost.last`, `SecretsError::DotenvRead.source`,
  `SecretsError::KeyringRead.source` and `ParseCause::Redacted` (ADR-SAG-0019, to clear
  Clippy's `result_large_err`). Optional fields keep their `Option` wrapper. JSON output
  and the Python wheel are unaffected.
- **0.1.0 detail JSON still reads.** NFR-SAG-0002 pins compatibility for detail payloads
  written by 0.1.0; the new fields are additive and omitted when unset.
- **Python 3.11 is the only floor.** 0.1.x's per-package floors are gone; a 3.10
  interpreter will not install this wheel.
- **Prefer `category` over message text.** The category set is closed and tested per
  crate; message text is not a stable interface and will change.
- **v0.2.8 is not a stable release.** The 0.2.8 prereleases carried a publish-time
  failure — self dev-dependencies declared with versions — that made the published
  crates unusable. v0.2.9 declares them by path so `cargo publish` strips them. If you
  pinned a `prerelease/v0.2.8` or `release-candidate-v0.2.8` artifact, move to v0.2.9.
