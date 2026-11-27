# Decision: sc-decidr is the one Python decision harness: error values with a stored category, per-call limits and a generated consumer module

**Status:** Approved  
**Version:** 2.0.0  
**Created:** 2026-11-14  
**Last Updated:** 2026-11-26  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-14  

---

## ADR-SAG-0019: sc-decidr is the one Python decision harness

**Status:** Approved  

### Related Documents

New with this ADR: REQ-SAG-0016 (typed error fields), REQ-SAG-0017 (categories), REQ-SAG-0702 (limits and batch order), REQ-SAG-0018 (the generated consumer module) and NFR-SAG-0003 (one Python floor). ADR-SAG-0004 sets the crate layering and that errors are `Result<T, E>`; ADR-SAG-0021 sets the log paths and the session sink.

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
- REQ-SAG-0005
- REQ-SAG-0703
- REQ-SAG-0101
- REQ-SAG-0102
- REQ-SAG-0103
- REQ-SAG-0801
- REQ-SAG-0019
- NFR-SAG-0004

**Architecture Decisions:**

- ADR-SAG-0004
- ADR-SAG-0021

### Context

sc-decidr is the harness through which sc-agent's Python consumers reach decision models (Jev, decider, Laya, Haiku, Luna). The owner requires judge-jev, plan-anchor and every later consumer to integrate identically for errors, API keys, model selection, the injectable general logger and session logging, differing only in prompt authoring and result handling.

The two existing consumers each rebuilt the wheel boundary, and the copies diverged: judge-jev pins an id list with a `JEV.UNMAPPED` fallback while plan-anchor matches message text (`"http 401"`); plan-anchor pins `provider="typesafe"`, disabling Haiku and Luna; both re-sort `send_batch` results; judge-jev refuses `--timeout` while plan-anchor calls the unreleased `timeout_secs`. Consumers had no category to branch on and no per-call worker count or timeout.

### Decision

The wheel is the one consumer boundary. Decision numbers are stable; there is no decision 3.

1. **The wheel is the consumer boundary.** Every consumer concern except prompt authoring and result interpretation belongs in sc-decidr: model selection, API keys (environment, repo-root `.env`, OS keyring), session logging, the general Logger, errors with a stored category (decisions 2, 5 and 6), limits and cancellation (decisions 7 and 14) and position-tagged batch results (decision 8). Session logging is always on: one row per query whose model call succeeded (a failed call writes no row; its error goes to the general log), at the wheel's default location, tagged with `project` and `agent_role`, written by sc-agent's session sink (ADR-SAG-0021); a cache hit writes no row and one link record (REQ-SAG-0005, REQ-SAG-0703). All Jev queries and responses therefore take one path.

   **The integration artifact is a generated module** (REQ-SAG-0018), rendered by each consumer from the sc-compose template `sc_agent/templates/consumer.py.j2`, shipped as package data; the wheel moves to maturin's mixed layout and `import sc_agent` is unchanged.

   - The template covers the decision call, model pass-through, logging wiring, cancellation and the error-to-category-to-code mapping.
   - model selectors are Run arguments.
   - A result line holds only the generic result fields; session data stays in the session row. The module reads session rows only through `sc_agent.session_rows`, finding a run's queries through its link records or the session_ids in its result lines, and never computes a session-log path.
   - Python results are typed values, not dicts.
   - sc-compose is an external render tool, not a wheel dependency; the template's front matter declares its sc-compose minimum (`requires.cli`), declared, not enforced.

   **The lower-level API stays the contract.** `Run` (with `send` and `send_batch`), `session_rows`, `provider_keys`, `TYPESAFE`, `digest`, `logger`, `log`, `flush` and `sc_agent.sitr` are public, and the generated code uses only them and the standard library.

2. **Typed fields: `attempts` and `category`.** An error value carries at least `id`, `message` and `action`, plus `attempts` (sends made, the first included) and `category` (decision 6) when they apply; unset fields are not written (REQ-SAG-0016). Retry-After is read as a Duration and used for the wait before a resend (REQ-SAG-0601).

5. **Python errors.** Every wheel call returns ok with a value or an error with its id and message; nothing raises (REQ-SAG-0016).

6. **The category is stored when the error is built.** The categories are `busy`, `quota`, `auth`, `network`, `response_invalid`, `validation`, `cancelled`, `config`, `log` and `other`. Every error has a stable id and stores its category, which Python reads from the `category` field (REQ-SAG-0017). Retryable means worth resubmitting as a new call later, after the wheel's own resends have run out; it is true for `busy` and `network` only, and a consumer that resubmits waits first rather than wrapping the wheel in an immediate retry loop.

7. **Per-run limits: `timeout_secs` and `workers`.** A Run takes an optional timeout (a Duration) and worker count, for that Run's calls only (`--timeout-secs` and `--workers` on the CLI); `None` means the configured value, and every send, resends included, gets the full timeout. The timeout defaults to 120 s; REQ-SAG-0702 holds the rules.

8. **`send_batch` returns one result per request, each with its input position**, while `on_result` still receives each line as it finishes (REQ-SAG-0702).

11. **sitr is an optional, separate section of the consumer template.** It uses `sc_agent.sitr` only (ADR-SAG-0005, ADR-SAG-0021) and is kept separate from the decision call: extraction runs outside the decision path, and its output feeds the consumer's prompt authoring. In this phase one offline test shows extraction output reaching a dry-run request.

14. **A run is a context object.** `sc_agent.Run(tool=, tool_version=, cache=, ...)` is created once per tool command and closed when its calls are done (`with sc_agent.Run(...) as run:`); creating it mints the run's ULID `run_id` in Rust (as are `session_id` ULIDs), readable as `run.run_id`.

### Rationale

Two consumers rebuilt the boundary and diverged, so the boundary moves into the wheel and integration code is generated from one template. A stored category and the send count replace message parsing and pinned id lists, and per-run limits and position-tagged results remove consumer workarounds.

### Consequences

**Positive:**

- Every Python consumer renders the same integration module and holds only its prompts and result handling.
- No consumer parses error text or pins id lists, and all Jev data takes one session-log path.

**Negative:**

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
