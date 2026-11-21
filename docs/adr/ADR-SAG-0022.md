# Decision: Limit sc-agent-log to session identity and session rows

**Status:** Proposed  
**Version:** 0.1.0  
**Created:** 2026-11-19  
**Last Updated:** 2026-11-20  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-19  

---

## ADR-SAG-0022: Limit sc-agent-log to session identity and session rows

**Status:** Proposed

### Related Documents

**Requirements:**

- REQ-SAG-0004
- REQ-SAG-0005
- REQ-SAG-0008
- REQ-SAG-0022

**Architecture Decisions:**

- ADR-SAG-0002
- ADR-SAG-0021

### Context

sc-agent uses the Rust sc-observability crate for general and session logs. The owner requires the smallest implementation that provides the stated logging design: the entry point owns one general Logger per process, session logs are a separate Logger, and there is no Python logging package (ADR-SAG-0021). The sc-decidr wheel holds both the decidr and the sitr Python APIs, so one compiled module links sc-observability.

The owner states: "general log failure is completely outside this concern. unless we are improperly using the general log, we expect it to work." The owner also states: "I don't want things forwarded to python logger. I want the default sc-observability logger to writ to disk."

**Problem Statement:**

sc-agent-log includes wrappers, error and loss accounting, and Python logger integration beyond the session-specific work sc-agent needs.

### Decision

sc-agent-log contains only:

1. Repository identity: normalize the git `origin` URL to `<host>/<owner>/<repo>`, without a scheme or `.git`; outside a git repository use `local/<host>/<folder-name>` (REQ-SAG-0004).
2. Slug validation of `project` and `agent_role` (REQ-SAG-0004).
3. Opening the Rust session-file Logger at `<log_root>/sessions/<repo>/<agent_role>/<project>/sessions.jsonl`, never rotated, and adding the session identity fields (`repo`, `project`, `agent_role`) to each row the decision runner builds (REQ-SAG-0004, REQ-SAG-0005).

After `cargo fmt`, the total line count of `crates/sc-agent-log/src/*.rs` MUST NOT exceed 250.

**Bans and replacements:**

- B1: sc-agent-log MUST NOT wrap a running sc-observability Logger in `Writer` or count lost rows. Callers use the sc-observability Logger directly.
- B2: sc-agent-log MUST NOT define `LogError` or logging-failure error ids. General-log failures are outside sc-agent's concern.
- B3: sc-agent-log MUST NOT define `GeneralLogger`. The general logger is the Rust sc-observability Logger.
- B4: sc-agent-log MUST NOT define `Loggers` or an injected-or-default selector. The entry point supplies its Logger; with none, the CLI or wheel uses the default Logger (ADR-SAG-0021).
- B5: sc-agent-log MUST NOT define `MemoryLog` or a `testing` feature for it. Tests use a sc-observability sink.
- B6: sc-agent-log MUST NOT recognize or convert Python `sc_observability.Logger`, `AttachedLogger`, Python `logging`, or other Python-owned loggers. Remove that bridge and its `pyo3` feature. The wheel hands Python the Logger built from the sc-observability crate it links and accepts that same object back (REQ-SAG-0008); no callable carries records between modules.
- B7: sc-agent-log MUST NOT define `LogKind`, `LogContext`, or `name.rs`, or re-export other crates' items from `lib.rs`. A request id is a record field; callers use source-crate types directly.
- B8: sc-agent-log MUST NOT wrap the session Logger in a `SessionLogger` type with its own error or lifecycle. The session log is a second Rust sc-observability Logger; sc-agent-log supplies its path and row fields.

The generated client obtains its Logger from the wheel once, under its client name, and injects it into every decidr and sitr call (REQ-SAG-0022). The log root is `~/.sc-agent`; `<CLI>_LOG_ROOT` is a test override only (ADR-SAG-0021). The wheel does not depend on the sc-observability Python package. The only record-size limit is the Rust crate's 1 MiB limit.

### Rationale

The Rust crate already writes and retains logs. Keeping only session identity and row construction removes duplicated machinery while preserving one Logger per process.

### Consequences

**Positive:** Less code and one owner for each client's writer.

**Negative:** Callers and tests must migrate to the Rust Logger and its sink interfaces.

### Alternatives Considered

#### Alternative 1: Keep the wrappers

**Description:** Retain the existing general and session wrappers, loss accounting, and Python bridge.

**Why Rejected:** Roughly 1,300 lines handle failures the owner ruled out and support logger ownership choices the design does not have.
