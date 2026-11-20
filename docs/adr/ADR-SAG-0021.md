# Decision: Rust-owned client logging and separate session logs

**Status:** Proposed  
**Version:** 0.3.0  
**Created:** 2026-11-18  
**Last Updated:** 2026-11-19  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-19  

---

## ADR-SAG-0021: Rust-owned client logging and separate session logs

**Status:** Proposed  

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

Each client's general records must reach one Rust-owned Logger, and every log must land at a fixed, known path, with session logs never mixed with general logs.

### Decision

**Rust-owned client logger.** Each CLI uses the Rust sc-observability Logger it links. The sc-decidr wheel opens one Rust Logger named for the client and owns it. The generated client supplies its name and holds the returned handle; it does not own a Python Logger. The same Logger writes the client's decidr and sitr records to one file.

**Separate wheels.** sc-decidr and sitr are separate compiled wheels. The sc-decidr-owned handle is a callable that accepts records and writes them through its Rust Logger. The generated client passes it to both wheels; sitr uses its existing callable path. Sitr alone uses its own Rust Logger. Neither wheel depends on the sc-observability Python package.

**Session log.** Rust writes the same session log for every client. It is not injectable from Python.

**Paths.** The log root is `~/.sc-agent` and is not a configuration setting. One function returns a CLI's log root: its `<CLI>_LOG_ROOT` environment override when set (used by tests), else `~/.sc-agent`. The CLIs and the wheel call that function; nothing else computes a log path.

- A general log is `~/.sc-agent/logs/<name>.log.jsonl`, the path sc-observability 1.5.0 writes for a Logger of that name.
- Session logs are `~/.sc-agent/sessions/<repo>/<agent_role>/<project>/sessions.jsonl`. `sessions/` sits beside `logs/` and holds nothing but session logs.

| Entry point | General log | Session logs |
|---|---|---|
| `decidr` CLI | `logs/decidr.log.jsonl` | `sessions/<repo>/<agent_role>/<project>/sessions.jsonl` |
| `sitr` CLI | `logs/sitr.log.jsonl` | none |
| `plan-anchor` (generated client) | `logs/plan-anchor.log.jsonl` | `sessions/<repo>/plan-anchor/<project>/sessions.jsonl` |
| `judge-jev` (generated client) | `logs/judge-jev.log.jsonl`, holding its decidr and sitr records | `sessions/<repo>/judge-jev/<project>/sessions.jsonl` |

A plan that adds an entry point or changes its log includes an updated row in this ADR.

**Session identity.**

- `agent_role` is the agent or app making the request. A generated client hardcodes it to its product name; it is not a user setting.
- `project` is the project/phase, resolved by the configuration lookup of ADR-SAG-0020 (`--project`, `SC_PROJECT`, then the config files). It is required and has no default; a session without one is refused with a typed error naming every place it can be set. Usage, help and errors call it "project/phase".

**Errors.** An error is logged once, in Rust, with the same id and fields whatever the harness, so the general logs can be searched with one set of terms. It is logged again only when an outer layer adds information that helps debugging. A failure of the general log itself is not handled: sc-agent expects it to work.

**Key Principles:**

- One Rust-owned general Logger, one writer, and one file per client.
- Session logs and general logs are never written to the same folder tree.
- A session is never filed under a guessed project/phase.

### Rationale

The sc-decidr wheel owns the Logger and its handle, and the separate sitr wheel sends its records through that handle. This keeps one writer and needs no Python logging package. Fixed paths keep every log in one known place for audit, and a separate `sessions/` tree lets tools and the later SQLite load read every session with a path glob.

### Consequences

**Positive:**

- Libraries hold no logging policy: they take a Logger or use the default.
- `sessions/<repo>/<agent_role>/<project>/` selects one agent's work in one phase with a path glob.

**Negative:**

- Every caller must supply a project/phase before its first request.

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

Several owners of one file handle risk lost or duplicated records. The sc-decidr wheel owns the client Logger.

#### Alternative 3: One Python wheel for both APIs

**Description:** Combine the decidr and sitr APIs into one wheel.

**Why Rejected:** The owner requires two separately compiled wheels and a Rust-owned callable handle between them.
