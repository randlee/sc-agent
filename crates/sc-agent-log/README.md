# sc-agent-log

General and session loggers for sc-agent, over `sc-observability` 1.5.0.

- `GeneralLogger`: the general log, `<log_root>/logs/<cli>.log.jsonl`, one file per CLI;
  retention is sc-observability's default policy (REQ-SAG-0003).
- `SessionLogger`: one JSONL row per call, appended to
  `<log_root>/logs/<repo>/<agent_role>/<project>/sessions.jsonl` and kept indefinitely
  (REQ-SAG-0004, REQ-SAG-0005).
- `Loggers::open`: inject either logger, both or neither (REQ-SAG-0008).
- `LogContext`: the session id and batch position of a general event (RSH-009).
- Feature `testing`: `testing::MemoryLog`, an in-memory sink for consumers' tests.

Every error is a `LogError` with a stable `log.*` id (`sc_agent_types::ErrorId`).
