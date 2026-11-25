# Decision: One sc-observability Logger per process, owned by the entry point; general logs and session logs in separate trees

**Status:** Approved  
**Version:** 0.5.0  
**Created:** 2026-11-18  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-19  

---

## ADR-SAG-0021: One sc-observability Logger per process, owned by the entry point; general logs and session logs in separate trees

**Status:** Approved  

### Related Documents

**Requirements:**

- REQ-SAG-0003
- REQ-SAG-0004
- REQ-SAG-0005
- REQ-SAG-0008
- REQ-SAG-0022

**Architecture Decisions:**

- ADR-SAG-0002
- ADR-SAG-0019
- ADR-SAG-0020

### Context

sc-agent logs through sc-observability. The `decidr` CLI, the `sitr` CLI and the sc-decidr wheel already link the sc-observability crate. Two kinds of log exist: the general log (diagnostics, searched for issues) and session logs (the long-term record of every model query and response, analysed later and loaded into SQLite, ADR-SAG-0002).

**Problem Statement:**

Every general-log record of a process must reach one Logger that the process's entry point owns, and every log must land at a fixed, known path, with session logs never mixed with general logs.

### Decision

**One Logger per process, owned by the entry point.** The entry point is the CLI's `main`, or, for a Python app, its sc-compose generated client. The entry point obtains one sc-observability `Logger`, keeps it for the life of the process, injects it into every sc-agent call, and flushes it after each call; the generated client has no close operation. Library code never builds a Logger of its own while one is injected, and never forwards records to Python `logging`.

**The wheel exposes the crate's Logger.** The sc-decidr wheel returns the sc-observability crate's `Logger` to Python, named by the caller. Python may write its own records to it and injects it into every sc-decidr call; Rust receives its own Logger object back, so no record crosses a bridge and the process has one queue. The wheel has no dependency on the `sc-observability` Python package.

**Products.** sc-agent ships three products. sc-observability is a crate linked into each, not a product.

| Product | Contains |
|---|---|
| `decidr` CLI | decidr, sc-observability |
| `sitr` CLI | sitr, sc-observability |
| sc-decidr wheel | the decidr and sitr Python APIs, sc-observability |

There is one Python wheel. A Python process that loads sc-agent has one copy of sc-observability and one Logger, which it injects into decidr and sitr calls alike. Installing the wheel always installs both APIs, whichever one the caller uses. There is no separate sitr wheel in phase h; a standalone sitr wheel may return after the MVP, and a process uses either it or the sc-decidr wheel, never both.

**Defaults.** With no Logger injected, sc-agent uses the default sc-observability Logger, which writes the general log to disk at the paths below. Every decision and sitr call is logged, so with nothing configured every log lands under `~/.sc-agent/` for a full audit. A caller that wants another general-log sink (a database, nothing) changes the sink of the Logger it injects; that is the caller's responsibility.

**Session log.** The session log is always on and is not configurable. sc-agent writes every session row in Rust through its own internal sc-observability session Logger, whose sink appends to the session file at the path below. That sink never rotates or prunes the file. Callers inject no session Logger and cannot replace its sink.

**Paths.** The log root is `~/.sc-agent` and is not a configuration setting. One function returns a CLI's log root: its `<CLI>_LOG_ROOT` environment override when set (used by tests), else `~/.sc-agent`. The CLIs and the wheel call that function; nothing else computes a log path.

- A general log is `~/.sc-agent/logs/<name>.log.jsonl`, the path sc-observability 1.5.0 writes for a Logger of that name.
- Session logs are `~/.sc-agent/sessions/<repo>/<agent_role>/<project>/sessions.jsonl`. `sessions/` sits beside `logs/` and holds nothing but session logs.

| Entry point | General log | Session logs |
|---|---|---|
| `decidr` CLI | `logs/decidr.log.jsonl` | `sessions/<repo>/<agent_role>/<project>/sessions.jsonl` |
| `sitr` CLI | `logs/sitr.log.jsonl` | none |
| `plan-anchor` (generated client) | `logs/plan-anchor.log.jsonl` | `sessions/<repo>/plan-anchor/<project>/sessions.jsonl` |
| `judge-jev` (generated client) | `logs/judge-jev.log.jsonl`, holding its decidr and sitr records | `sessions/<repo>/judge-jev/<project>/sessions.jsonl` |

A wheel is not an entry point and has no row: it writes to the Logger its caller injects. Every entry point has a row and every cell is filled. The table is updated when an entry point is added.

**Session identity.**

- `agent_role` is the agent or app making the request. A generated client hardcodes it to its product name; it is not a user setting.
- `tool` and `tool_version` name the program that sent the query and its version: the generated client's name, or `decidr` (REQ-SAG-0004).
- `project` is the project/phase, resolved by the configuration lookup of ADR-SAG-0020 (`--project`, `SC_PROJECT`, then the config files). It is required and has no default; a session without one is refused with a typed error naming every place it can be set. Usage, help and errors call it "project/phase".

**Errors.** An error is logged once, in Rust, with the same id and fields whatever the harness, so the general logs can be searched with one set of terms. It is logged again only when an outer layer adds information that helps debugging. A failed `log.write` or `log.flush` returns the Logger's failure as an error value; nothing else handles it. A successful decision remains in its session row when general logging fails; the call returns the log error rather than that decision value. A session row is written as one sc-observability event of at most 1 MiB, the event wrapper included; the largest request that fits is recorded whole. When a session event is larger than 1 MiB, or the session write fails, no row is written and the call returns a `log.session_write` error.

**Key Principles:**

- One process, one general Logger, one queue, owned by the entry point; session rows go through sc-agent's own session Logger, never the general Logger.
- Session logs and general logs are never written to the same folder tree.
- A session is never filed under a guessed project/phase.

### Rationale

The entry point is the only code that knows the process's lifetime, so it is the only correct owner of the Logger: it obtains the Logger before the first general-log record and flushes after each call. Exposing the crate's own Logger from the wheel keeps one copy and one queue in the process and needs no Python package. Fixed paths keep every log in one known place for audit, and a separate `sessions/` tree lets tools and the later SQLite load read every session with a path glob. The session log is the record of every Jev query from every tool, so it cannot be turned off and is never rotated; it stays small enough because `project` changes over time.

Owner ruling (2026-11-21), Rand Lee: "My words are what I want, not excess control resulting." "I was planning to bring the sitr wheel back once we get mvp working."

Owner rulings (2026-11-24), Rand Lee, recorded in `docs/plans/phase-j/audit-trail-log-normalization-SQLite.md`: the session log is written in Rust through sc-observability, which accepts events up to 1 MiB; the session log is never rotated or pruned.

### Consequences

**Positive:**

- Libraries hold no logging policy: they take a Logger or use the default.
- `sessions/<repo>/<agent_role>/<project>/` selects one agent's work in one phase with a path glob.

**Negative:**

- Every caller must supply a project/phase before its first request.
- A query whose session event exceeds 1 MiB gets no session row.

### Alternatives Considered

#### Alternative 1: Python creates the Logger from the `sc-observability` Python package

**Description:**

The client imports `sc_observability`, creates a Logger, and sc-agent calls its `log(LogEvent)` through Python.

**Why Rejected:**

It adds a Python dependency for a crate the wheel already contains, a second copy of sc-observability in the process, and a bridge for every record.

#### Alternative 2: The library opens its own Logger when none is injected, and the CLI opens another

**Description:**

Each layer opens and closes loggers as it needs them.

**Why Rejected:**

Several owners of one file handle; records are lost or duplicated on interrupt. The entry point owns the Logger.

#### Alternative 3: A separate sitr wheel

**Description:**

Ship decidr and sitr as two Python wheels.

**Why Rejected:**

Each wheel carries its own copy of sc-observability, so a process loading both cannot inject one Logger into both without a bridge. One wheel gives one Logger.

#### Alternative 4: Log root as a configuration setting

**Description:**

`log_root` in the configuration files and on the command line.

**Why Rejected:**

Not requested. Logs live in one known place; tests use `<CLI>_LOG_ROOT`.
