# sc-agent reduction plan

Owner: Rand Lee. 2026-11-19.

## Goal

sc-agent replaces about 300 lines of per-tool Python (judge-jev `jev_judge.py.j2` at `0780e60^`: `call_jev`, `http_error`, `run_batch_job`, `stream_batch`/`pump_batch`, the key lookup, the run log) with one Rust core that every tool shares, plus sitr. Today it is about 33,000 lines of Rust source and 23,000 lines of tests in 17 crates. Target: code comparable to the 300 lines of Python plus the model catalog, no unnecessary wrapping, no exceptions thrown. Budget: **about 2,000 lines of Rust in total (failing above 2,200) across every `.rs` file in the repository, tests, examples and scripts included**. Until the old crates are deleted, the gate counts only the new crates; after deletion it counts the whole repository.

**Wheel.** One Python wheel (`sc-decidr`) holding the decidr and sitr APIs. `logger(name)` returns its Rust Logger, which every decidr and sitr call accepts directly; no callable bridge. The `sitr` and `decidr` CLIs each use their own Logger.

**No unnecessary wrapping:** ureq, keyring, dotenvy and sc-observability are called directly. No traits, no newtypes or wrapper structs over library types, no re-exports. A type exists only when it carries data the libraries do not (a provider table row, a result, an error).

**No exceptions thrown:** every wheel function returns a result value (`{ok, value}` or `{ok: false, error: {id, message, action}}`) and never raises; the generated client never raises. Rust non-test code has no `unwrap`, `expect`, `panic!` or indexing that can panic (clippy `unwrap_used`, `expect_used`, `panic`, `indexing_slicing` denied).

## Method: rewrite to a spec, do not trim

Trimming the old code in place keeps its structure (17 crates, traits, wrappers) and moves a few hundred lines per pass. The new code is written fresh on one branch from the spec below, then the old crates are deleted in one commit.

## Spec (the only inputs)

1. The records at commit `f023c0d` (ADR-SAG-0021 0.3.0 and the records it aligns).
2. The original Python: judge-jev `plugins/sc-judge-jev/install/templates/skills/judge-jev/scripts/jev_judge.py.j2` at `0780e60^` (lines 794-840, 1006-1041, 1307-1380, 1456-1660), and plan-anchor `decidr_client.py.j2` at `fff69b2`.
3. This file.

Nothing not in these three is built.

## What sc-agent does

- **send / send_batch.** One request, or many with a worker fan-out and a per-call timeout. Result or typed error per request.
- **Models: one table per provider**, static, in code:

  | Provider | Base URL | Key variable | Request shape | Models (short name → id, release date) |
  |---|---|---|---|---|
  | typesafe | `https://api.typesafe.ai/v1` | `TYPESAFE_API_KEY` | SystemOne | jev → `jev-1.14.0`, decider → (current id) |
  | anthropic | `https://api.anthropic.com/v1` | `ANTHROPIC_API_KEY` | Messages | haiku → `claude-haiku-5-5` |
  | openai | `https://api.openai.com/v1` | `OPENAI_API_KEY` | chat/completions | luna → `gpt-6-luna` |
  | openrouter | `https://openrouter.ai/api/v1` | `OPENROUTER_API_KEY` | chat/completions | the Claude and GPT short names, with OpenRouter's ids |
  | together | `https://api.together.ai/v1` | `TOGETHER_API_KEY` | chat/completions | the Claude and GPT short names, with Together's ids |
  | nous | `https://inference-api.nousresearch.com/v1` | `NOUS_API_KEY` | chat/completions | none named; ids passed through |
  | local | host:port from config | none | SystemOne | laya |

  Each provider table lists its models as (short name, model id, release date). `send(model, provider=None)`: with a provider, look in that table; without, the first table in list order that has the name. When several rows share a short name, the one with the most recent date is used by default; a full model id selects that row exactly. With a reseller provider (openrouter, together, nous) and a name in no table, the text is sent as the model id unchanged. No provider ranking and no config catalog: the tables are static code. All six providers the current code supports are kept; the reseller model ids are taken from the current code's tables.
- **Retry.** Busy (429/502/529) and network errors are resent up to a fixed count, waiting `Retry-After` when given, else a fixed delay; quota-exhausted bodies are not resent. The wait goes through an injected sleep function so tests never sleep.
- **Keys.** Environment variable, then repo-root `.env` (via `dotenvy`, never written to the process environment), then the OS keyring (via `keyring`, read on a thread with a 15 s `recv_timeout` because keyring has no timeout). The key is a plain `String` that is never put into an error, a log record or a Debug output (a contract test checks the key text appears nowhere).
- **Config.** sc-config 0.2.0's `Loader`, used as it is (its files and prefixed environment variables). Settings: `project`, `workers`, and the Laya runner's `host`/`port`/`model`. `--project` on the CLI overrides. `decidr doctor` prints each setting and value, and its source when sc-config reports one.
- **Logging (ADR-SAG-0021 0.3.0).** One sc-observability `Logger` per process, owned by the entry point; injected into every call; default Logger writes `~/.sc-agent/logs/<name>.log.jsonl`; session rows to `~/.sc-agent/sessions/<repo>/<agent_role>/<project>/sessions.jsonl`; `<CLI>_LOG_ROOT` test override; errors logged once with the same ids everywhere; general-log failure not handled.
- **Errors.** One error enum for the core with stable ids and an action string, logged and returned the same way from the CLI (non-zero exit plus the JSON error) and the wheel (a returned error value, never a raised exception).
- **Products.** `decidr` CLI (`send`, `models` = print the tables, `doctor` = setting/value/source), `sitr` CLI (its existing commands), and one `sc-decidr` wheel (decidr API, sitr API, `logger(name)` returning the Rust Logger both accept). No `sc-observability` Python dependency, no callable bridge, no forwarding to Python `logging`.
- **Generated client.** The sc-compose template: obtains one Logger by client name, injects it into every call, hardcodes `agent_role`.
- **Interrupt.** A cancel flag (about 10 lines): Ctrl-C stops sending new requests and the run returns what finished.

- **Also included.**
  - Cache key: the per-question sha256, with a public function to compute it.
  - Each session row carries the request text.
  - A distinct typed answer per question type.
  - Layered config through the `sc-config` crate, called directly (no loader of our own).
  - Cancel (Ctrl-C stops new requests; finished results are returned).
  - `decidr doctor`.

## Not built

Image parts, the long-context header, provider ranking, the user model catalog, keyring location overrides, `VarName` key indirection, a layered config loader of our own (sc-config is used), logger wrappers, lost-row accounting, logging-failure error types, forwarding to Python `logging`, the `Transport` trait and its test doubles, wrapper request/response types, the `Clock` trait (an injected sleep function replaces it), a separate sitr wheel, a callable logger bridge.

## Layout

| Crate | Contents | Guide budget (Rust lines after `cargo fmt`, tests included; the hard limit is the 2,200 total) |
|---|---|---|
| `sc-agent` | provider tables (model catalog), send/send_batch, retry, keys, config, errors, logging paths, session rows | 700 |
| `decidr` | CLI | 150 |
| `sc-agent-py` | the one wheel: decidr API, sitr API, `logger(name)`; returns results, never raises | 250 |
| `sc-treesitter` + `sitr` | tree-sitter extraction and its CLI, rewritten the same way | 800 |

Dependencies allowed: ureq, serde, serde_json, keyring, dotenvy, ulid, sc-sha, sc-config, ctrlc, sc-observability, clap, pyo3, tree-sitter and its grammars, thiserror. Anything else fails the gate.

## Gates (one script, seconds, run before a task can close and before push)

1. Total about 2,000 Rust lines (tests, examples, scripts included; after `cargo fmt`), failing above 2,200; new crates only until the old crates are deleted.
2. Dependency allow-list (`cargo metadata`).
3. Banned names absent: `Transport`, `Clock`, `CancelToken`, `Catalog`, `LoggerBuilder` outside the default-Logger function, `sc_observability` in any `pyproject.toml` dependency list.
4. Clippy with `unwrap_used`, `expect_used`, `panic`, `indexing_slicing` denied in non-test code.
5. No `raise` in the wheel's Python or the generated client; no `PyErr` returned from a wheel function.
6. Contract tests pass (below).
The gate script and the budgets live in a path only the owner changes.

## Contract tests (black box, at the product boundary)

Against a local HTTP server in the test, no network, no real home, no sleeps:
- send to each provider's request shape returns the parsed result; a 429 with `Retry-After` is resent and succeeds; a quota body is not resent; a 401 gives the auth error id.
- send_batch with workers=2 returns one result per request, matched by index.
- key lookup order (env, `.env`, keyring double) and the never-printed key.
- `project` lookup order and the missing-project error naming every place.
- log files land at the fixed paths with the fixed fields; with a Logger injected, nothing is written to the default path; Python obtains `logger("t")`, logs to it, passes the handle to a decidr call and a sitr call, and finds all records in `t.log.jsonl`; sitr alone writes `sitr.log.jsonl`.
- every wheel function, given bad input, a provider error and a network error, returns an error value and raises nothing.
- `decidr send`, `decidr models`, `decidr doctor`, and the `sitr` commands.
Existing boundary tests in `crates/decidr/tests/` and `crates/sc-agent-py/tests/` are mined for cases; internal-type tests are not carried.

## Execution

1. One developer agent, one branch `rewrite/v1` cut from `develop`, one worktree. Input: this file and the spec. No beads per deliverable, no stack, no QA panel.
2. Order: gate script and contract tests first (they fail), then the code until they pass within budget.
3. Review: one deletion reviewer ("what here can be removed or replaced by a library call") and one spec check by the lead against this file. Findings are fixed in place on the same branch; a fix may delete code.
4. Done when the gates pass and plan-anchor's and judge-jev's suites pass against the new wheel. Then one PR into `develop` that also deletes the old crates; the owner merges.

## Stopped

- tai's in-place trim on `work/rust-under-2k` (it kept two wheels and added a callable bridge): stopped, nothing committed.
- PR #324 (reverted the one-wheel and no-callable decisions): superseded by this plan.
- Phase-e stack #319 and bead `agt-next-1`: superseded; not landed.
