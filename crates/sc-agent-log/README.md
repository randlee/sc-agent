# sc-agent-log

General and session loggers for sc-agent, over `sc-observability` 1.5.0. It sits in the config, secrets and logging layer, above `sc-agent-types`. Both loggers take generic rows (`serde_json::Map`), so the crate has no decision types. The runner builds the decision rows.

## What it provides

- `GeneralLogger`: the diagnostic log, `<log_root>/logs/<service>.log.jsonl`, one file per CLI. Retention is sc-observability's default policy.
- `SessionLogger`: one JSONL row per call, appended to `<log_root>/sessions/<repo>/<agent_role>/<project>/sessions.jsonl` and kept indefinitely. Every row starts with `session_id`, `repo`, `project` and `agent_role`.
- `Loggers::open`: opens both. Either can be an injected `Arc<Logger>`; what is not injected gets the default.
- `SessionIdentity`, `Repo` (`Repo::detect`, `Repo::local`, `Repo::work_tree_root`), `Slug` and `LogContext` (the session id and batch position of a general event).
- `LogError`. `flush()` reports every refused or lost row once as `LogError::RowsLost`; `close()` flushes, shuts down and returns the same report.
- `try_write` on both loggers for callers that must not block on a full queue. `MAX_ROW_BYTES` and `QUEUE_CAPACITY` are the limits.
- Re-exports of sc-observability's `Logger` and `Level`.

## Usage

```rust,ignore
use sc_agent_log::{Loggers, Repo, SessionIdentity};
use sc_agent_types::SessionId;

let repo = Repo::local("m5", "work")?;
let identity = SessionIdentity::new(repo, "phase-b", "sanity")?;
let loggers = Loggers::open(log_root, "decidr", identity, None, None)?;

loggers.session.write(SessionId::new(), serde_json::Map::new())?;
loggers.session.flush()?;
loggers.close()?;
```

`log_root` is a `&Path`. Callers take it from `cli_log_root` (re-exported from sc-agent-types): `<CLI>_LOG_ROOT`, else `~/.sc-agent`.

## Python callable records

An injected Python callable receives one dictionary per event. Its keys are:

- `level`, `target`, `action`: strings; level is `trace`, `debug`, `info`, `warn`, or `error`.
- `message`, `request_id`, `correlation_id`, `outcome`: strings or `None`. For a session event, `correlation_id` is the session ID.
- `trace`: a JSON-compatible dictionary or `None`.
- `fields`: a dictionary containing JSON-compatible values.

The keys are always present; fields without a value are `None`. The callable is caller-owned.

## Workspace test linking

The Linux all-features workspace test unifies the `python` feature into binaries
such as `decidr`, so CI adds setup-python's `LIBDIR` to `LD_LIBRARY_PATH` for
that test job. A standalone `cargo build -p decidr` does not enable that feature
and does not require Python at runtime.

## Features

- `python` (not default): `Injected`, `Loggers::open_with`, and direct injection of caller-owned Python `Logger`, `AttachedLogger`, or callable. The caller retains flush and shutdown.
  `Injected::Python` exposes `pyo3::Py<PyAny>`, so the pinned PyO3 version (0.29.2) is part of this feature's Rust API compatibility contract. Consumers passing Python objects must use a compatible PyO3 version; changing it can break their builds.
- `testing` (not default): `testing::MemoryLog`, an in-memory sink for consumers' tests. It adds no dependency.

## Depends on

- `sc-agent-types`.

Layer rule: a crate depends only on lower layers (ADR-SAG-0004). The crate starts no runtime and never sleeps.

## Requirements and decisions

- REQ-SAG-0003: log retention.
- REQ-SAG-0004: session log identity (repo, agent role and project).
- REQ-SAG-0005: session log JSONL layout.
- REQ-SAG-0008: optional logger injection with default loggers.
- REQ-SAG-0009 and REQ-SAG-0010: no tokio, no sleep.
- NFR-SAG-0001: session logging at 40,000 sessions per task.
- ADR-SAG-0002: session logs are JSONL first.
- ADR-SAG-0004: crate layering.
