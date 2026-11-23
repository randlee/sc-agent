# sc-agent v0.4.0 — Five Crates, One Wheel, Results Instead of Exceptions

**Released:** November 23, 2026 · **Tag:** `v0.4.0` · **Previous stable:** `v0.2.9` (November 18, 2026)

**Install:** `pip install sc-decidr==0.4.0` (one wheel: `sc_agent` and `sc_agent.sitr`), `cargo install decidr` (crates.io), `brew install randlee/tap/decidr`, `scoop install randlee/decidr`, `winget install --id randlee.decidr --exact`, or the platform archives on the [release page](https://github.com/randlee/sc-agent/releases/tag/v0.4.0)

[README](https://github.com/randlee/sc-agent/blob/main/README.md) · [Requirements](https://github.com/randlee/sc-agent/tree/main/docs/requirements) · [ADRs](https://github.com/randlee/sc-agent/tree/main/docs/adr) · [Release notes](https://github.com/randlee/sc-agent/releases/tag/v0.4.0)

This is the rewrite release. The workspace went from seventeen crates to five —
`sc-agent`, `decidr`, `sc-treesitter`, `sitr`, `sc-agent-py` — and the Python
side went from two wheels to one. It is also the first release where sc-agent
crates ship on crates.io, because the `sc-config` dependency it pins is now
published at 0.3.0 instead of reachable only by Git revision. The interface is
familiar: `decidr send`, `sc_agent.send`, `send_batch`, `digest`, `sitr`, and
`doctor` do the same work. What changed is what a call *returns*, how a consumer
obtains a logger and a cancel handle, and where configuration and logs live.

## Python Integrator — every call returns a value, nothing raises

**As a Python integrator, I want the error surface typed and the package properly marked, so that my type checker and my `except` clauses agree with what the library actually raises.**

This is the break that will touch every Python consumer. The wheel no longer
raises `SagError`. Every function in `sc_agent` — `send`, `send_batch`, `digest`,
`logger`, `log`, `flush`, and the `sitr` queries — returns an envelope:

```python
{"ok": True,  "value": ...}
{"ok": False, "error": {"id": "...", "message": "...", "action": "...", "category": "...", ...}}
```

The stub ships the shape, so `mypy` and `pyright` see the same contract the
interpreter enforces: `Envelope = dict[str, Any]`, and the returned error object
keeps the typed fields 0.2.9 introduced — `id`, `message`, `category`,
`status`, `attempts`, `retry_after_ms`. A `try/except` around a decision call
now catches nothing; branch on `result["ok"]` and read `result["error"]`
instead. The generated consumer template already does this, so a consumer
rendered from the shipped template needs no hand-written error handling.

Two other Python-facing changes come with the single wheel. `sc_agent.Cancel()`
replaces the old `CancelToken` — construct it, hand it to a call as `cancel=`,
and call `.cancel()` to stop work; a keyboard interrupt cancels unfinished work
and returns a cancellation envelope rather than propagating. And logging is one
native Logger shared across the process: `sc_agent.logger(name)` returns a handle
whose `value` you pass as `general_log=` to every decision, batch and sitr call,
instead of each call building its own logger. The Python floor stays 3.11.

## Agent-Run Author — one wheel to install, one template to render

**As an agent-run author, I want one generated consumer module that already knows the error contract, so that wiring a new pipeline to sc-agent is a render and an import instead of a copied helper.**

`sitr` is no longer a separate wheel. `sc-decidr` 0.4.0 carries both the decision
API and the tree-sitter API under `sc_agent` and `sc_agent.sitr`, and ships
`sc_agent/templates/consumer.py.j2` as package data, so `importlib.resources`
finds the template on an installed wheel rather than only in a source checkout.
Rendering the template with `client_name` and `sitr` settings produces a module
that handles returned envelopes, owns a `Cancel`, and creates exactly one named
Logger to inject into its decision and sitr calls — so a consumer no longer
threads a logger per call.

If you have the old separate wheel installed, the migration is two commands:
`pip uninstall sitr` then `pip install sc-decidr`. `import sc_treesitter`
becomes `import sc_agent.sitr`; the `sitr` CLI still exists and comes from the
binary archives or a package manager, and both `decidr --version` and
`sitr --version` now work.

## Pipeline Orchestrator — a role on the record, a cancel you hold

**As a pipeline orchestrator, I want every failure to carry its category, HTTP status, attempt count and Retry-After, so that retry, backoff and abort decisions are computed from fields instead of string matching.**

The typed failure contract from 0.2.9 survives the rewrite intact — the
categories (`busy`, `quota`, `auth`, `network`, `response_invalid`,
`validation`, `cancelled`, `config`, `log`, `other`), the wrapper that copies a
cause's fields to the top level, and `retry_after_ms` all still resolve from the
leaf variant rather than from message text. Two things are tightened around it.

A decision call now requires the agent role. `decidr send --agent-role` and
`sc_agent.send(..., agent_role=)` are no longer optional metadata: the role
selects the session tree and the log context, so it is validated up front and a
missing role is refused before any request leaves the process.

Cancellation is an explicit handle on both sides. Ctrl-C or SIGTERM cancels
unfinished work and returns a cancellation error with exit status 1; success
exits 0; there is no third-interrupt escape hatch. In-process, `sc_agent.Cancel`
(or the Rust equivalent) is passed per call, so an orchestrator can cancel one
call without tearing down the process, and unfinished batch work is drained
before shutdown rather than abandoned.

## Ops Engineer — layered config, a fixed log root, and a doctor that proves it

**As an ops engineer, I want a key or configuration fault to name its category and its trigger, so that a broken host is diagnosed from the error rather than from an experiment.**

Configuration is now the layered `sc-config` engine from sc-runtime 0.3.0, with
the precedence order stated instead of implied: CLI flag, environment variable,
local `decidr.local.json`, local `decidr.json`, local `sc-agent.json`, global
`decidr.local.json`, global `decidr.json`, global `sc-agent.json`, built-in
default. Local files live in `<worktree>/.sc/sc-agent/`; global files live in
`~/.sc-agent/`, relocatable with `SC_AGENT_HOME` or `--config-dir` (wheel:
`config_dir`). Environment values parse as JSON when they can, otherwise stay
strings, so a list or object no longer needs a stringly-typed workaround.
`decidr doctor` reports each value, its source, and each layer file's path and
existence — without sending a request.

Keys resolve in a fixed order: environment, then the repository-root `.env`, then
the OS keyring, first source that has the key wins. An ambiguous keyring
override is still refused before lookup, an attached key is still redacted from
all error text, and a keychain approval timeout is now reported distinctly from
a read failure so a hung prompt is not mistaken for a missing credential.

The log root is no longer a setting. `log_root` is removed; session rows and the
general log both use the fixed `~/.sc-agent` root (with `<NAME>_LOG_ROOT` as a
test-only environment override), and `doctor` reports the value. General logs are
`logs/<name>.log.jsonl`, rotated by sc-observability; session rows append to
`sessions/<repo>/<agent_role>/<project>/sessions.jsonl` and are never rotated or
pruned. Session records are preserved up to 1 MiB.

## Static-analysis Tool Builder — `sitr` in the same wheel, still off the decision path

**As a tool builder, I want tree-sitter extraction available inside the same generated client, so that a structural check and the decision call live in one module.**

`sc-treesitter` keeps its grammar and query semantics; what changed is packaging
and wiring. `sc_agent.sitr` exposes `languages()`, `queries()`, `query()` and
`run()`, all returning envelopes like the decision calls and all accepting the
same shared `general_log`. In the generated consumer, render with `sitr=true` and
the module gains `extract(language, query, source, captures=None)`; extraction is
only imported when called, so a consumer that never renders it pays nothing, and
the captures feed the request you author while the decision path stays unchanged.

---

## Upgrade Notes

- **Python results, not exceptions.** Wrap nothing; branch on `result["ok"]`.
  Any `except SagError` is dead code after this upgrade.
- **`CancelToken` → `sc_agent.Cancel`.** Construct per run, pass as `cancel=`.
- **Logger handle.** `sc_agent.logger(name)` returns an envelope; pass
  `handle["value"]` as `general_log=` to each call. One logger per process is the
  supported pattern.
- **`sitr` wheel retired.** `pip uninstall sitr && pip install sc-decidr`;
  `sc_treesitter` moves to `sc_agent.sitr` for Python. The CLIs are unchanged.
- **`agent_role` is required** on `send` and `send_batch`, in both the CLI and the
  wheel.
- **`log_root` is gone.** Use `~/.sc-agent` (or `SC_AGENT_HOME` for the config
  tree); there is no configuration key for the log root.
- **Crates.io is live.** `decidr` 0.4.0 and the `sc-agent` 0.4.0 crates publish
  conventionally now that `sc-config` 0.3.0 is on crates.io — no Git pin needed
  for the published artifacts. Building from the source distribution still needs
  Rust and Git access for the pinned `sc-config` dependency.
- **Jev model names.** Use `jev` (or an explicit `jev-1.14.0`); `jev-latest` is
  gone.
- **The workspace is synchronous.** No tokio, no async runtime; a consumer that
  expected to drive these calls from an async loop should call them from a worker
  thread rather than assume awaitable surfaces.
