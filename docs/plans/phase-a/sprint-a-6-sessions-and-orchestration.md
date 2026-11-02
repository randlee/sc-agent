---
id: a-6
phase: a
status: planned
closure_type: boundary
target_boundary: jev-check
track: runtime
wave: 3
branch: sprint/a-6-sessions-and-orchestration
worktree: sprint/a-6-sessions-and-orchestration
recommended_agent: lai
recommended_model: gpt-5.6-luna-medium
must_follow: [a-2, a-3, a-4, a-5]
parallel_safe: []
requirements:
  - REQ-SAG-0503
  - REQ-SAG-0600
  - REQ-SAG-0601
  - REQ-SAG-0602
  - REQ-SAG-0603
  - REQ-SAG-0610
  - REQ-SAG-0611
adrs:
  - ADR-SAG-0008
  - ADR-SAG-0009
  - ADR-SAG-0013
  - ADR-SAG-0016
  - ADR-SAG-0019
owned_paths:
  - "crates/sc-jev-check/src/session/**"
  - "crates/sc-jev-check/src/run/**"
  - "crates/sc-jev-check/src/equal_input/**"
  - "crates/sc-jev-check/src/registry/**"
---

# Sprint a-6 — sessions, run_request, multi-backend

## Outcome

Library orchestration: `run_check`, `run_request`, `SessionStore` with UUID
paths mode `0600`, expiry from `created_at`, `ask` on Haiku/Luna sessions,
equal-input record on every backend run, multi-backend in one invocation
(feature **3**), and **session logging** every prompt via **`SharedSessionLog`**
(v2 shim → real logger in `sc-ai`).

## Deliverables

1. **REQ-SAG-0600** — `SessionStore`: create/load, TTL, file permissions tighten **#6**, **#7**.
2. **REQ-SAG-0601** — `run_request` / `run_check` wire `resolve_key(_, repo_root)` + backends; load LLM system prompt file into `JudgeRequest.llm`; dry-run returns digest only (feature **2**).
3. **REQ-SAG-0602** — `EqualInputRecord` with `prompt_sha256`, `leak`, `commands`, `equal` per README.
4. **REQ-SAG-0603** — `MultiBackendRun`: Jev + optional second opinions in one call; shared payload bytes; shared injected LLM system prompt for that run.
5. **ADR-SAG-0008** — Sessions are how second backends receive prior answers; Jev has no rationale field.
6. **ADR-SAG-0009** — Local backend registers via trait without changing callers (stub registry only).
7. **REQ-SAG-0610** — Emit one `LogEvent` per judge send through injected `SharedSessionLog` (v2 port).
8. **REQ-SAG-0611** — Events include `session_id`, backend, `prompt_sha256`; never API keys.
9. **ADR-SAG-0013** / **ADR-SAG-0016** — No bespoke writers; libraries use port only.

## Explicit code samples

```rust
// EqualInputRecord defined in sc-agent-types (a-1)

pub trait SessionStore: Send + Sync {
    fn create(&self, backend: BackendId, bundle: &PayloadBundle) -> Result<SessionId, JevError>;
    fn ask(&self, id: &SessionId, follow_up: &str) -> Result<JudgeOutcome, JevError>;
}

pub fn run_check(
    spec: &CheckSpec,
    backends: &[BackendId],
    opts: RunOptions,
) -> Result<CheckResult, JevError>;
```

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0503 | Changing prompt file changes LLM wire without recompiling backends |
| req:REQ-SAG-0603 | Multi-backend run shares one `PayloadBundle` |
| req:REQ-SAG-0610 | Each send emits `LogEvent` via injected port |

## Required validation

```bash
just lint
cargo test -p sc-jev-check session -- --nocapture
cargo test -p sc-jev-check run -- --nocapture
cargo test -p sc-jev-check equal_input -- --nocapture
cargo test -p sc-jev-check -- test_session_round_trip --exact --nocapture
cargo test -p sc-jev-check -- test_equal_input_record --exact --nocapture
cargo test -p sc-jev-check -- test_multi_backend_one_payload --exact --nocapture
cargo test -p sc-jev-check observability -- --nocapture
```

Expected: exit 0; README parity rows for sessions, equal-input, multi-backend library paths; JSONL tests use temp dir sink.

## Parity checklist

See **plan.md** parity rows for sprint **a-6**.

## This sprint does not close

CLI wiring (a-7), PyO3 (a-8), `sessions list/show` (phase-b feature 5).
