# sc-agent-log

General and session loggers for sc-agent, over `sc-observability` 1.5.0. It sits in the config, secrets and logging layer, above `sc-agent-types`. Both loggers take generic rows (`serde_json::Map`), so the crate has no decision types. The runner builds the decision rows.

## What it provides

- `GeneralLogger`: the diagnostic log, `<log_root>/logs/<service>.log.jsonl`, one file per CLI. Retention is sc-observability's default policy.
- `SessionLogger`: one JSONL row per call, appended to `<log_root>/logs/<repo>/<agent_role>/<project>/sessions.jsonl` and kept indefinitely. Every row starts with `session_id`, `repo`, `project` and `agent_role`.
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
let loggers = Loggers::open(log_root, "decide", identity, None, None)?;

loggers.session.write(SessionId::new(), serde_json::Map::new())?;
loggers.session.flush()?;
loggers.close()?;
```

`log_root` is a `&Path`. It normally comes from `AgentConfig::log_root()`; this crate does not decide it.

## Features

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
