---
id: a-1
phase: a
status: planned
closure_type: contract
target_boundary: agent-types
track: foundation
wave: 1
branch: sprint/a-1-workspace-and-types
worktree: sprint/a-1-workspace-and-types
recommended_agent: rust-architect
recommended_model: opus
must_follow: []
parallel_safe: []
requirements:
  - REQ-SAG-0100
  - REQ-SAG-0101
  - REQ-SAG-0102
  - REQ-SAG-0103
  - REQ-SAG-0120
  - REQ-SAG-0121
  - NFR-SAG-0001
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0002
  - ADR-SAG-0016
  - ADR-SAG-0019
owned_paths:
  - "Cargo.toml"
  - "Cargo.lock"
  - "justfile"
  - "sc-lint.toml"
  - "boundaries/README.md"
  - "crates/sc-agent-types/**"
  - "crates/sc-agent-types/src/observability/**"
  - "tests/fixtures/jev/**"
  - "tests/jev_cross_impl/**"
  - "crates/sc-agent-secrets/Cargo.toml"
  - "crates/sc-agent-secrets/src/lib.rs"
  - "crates/sc-jev/Cargo.toml"
  - "crates/sc-jev/src/lib.rs"
  - "crates/sc-jev-backends/Cargo.toml"
  - "crates/sc-jev-backends/src/lib.rs"
  - "crates/sc-jev-check/Cargo.toml"
  - "crates/sc-ai/Cargo.toml"
  - "crates/sc-ai/src/main.rs"
  - "crates/sc-jev-py/Cargo.toml"
  - "crates/sc-jev-py/src/lib.rs"
---

# Sprint a-1 — workspace, sc-lint, errors, cross-impl fixture

## Outcome

Rust workspace exists with sc-lint boundaries, `sc-agent-types` error enum
(with fix text per variant), and a **cross-implementation** payload fixture
verified against committed Python reference bytes. **No network code.**

## Deliverables

1. **REQ-SAG-0100** — Root **`Cargo.toml` `[workspace.members]`** lists every Phase A crate:
   `sc-agent-types`, `sc-agent-secrets`, `sc-jev`, `sc-jev-backends`, `sc-jev-check`,
   `sc-ai`, `sc-jev-py`. a-1 commits **minimal compiling stubs** (`Cargo.toml` + root
   module only) for each member except `sc-agent-types` (fully implemented here).
   Later sprints **replace stub bodies** without editing root `Cargo.toml` members list.
   Exception: **`sc-jev-check/src/lib.rs`** module tree is owned by **a-3** (a-1 may
   add a one-line empty root only so `cargo check --workspace` passes).
   `just lint` / `just test`, sc-lint manifests for Phase A boundaries.
2. **REQ-SAG-0101** — `JevError` enum covering every id in jev-check README Errors table; each variant exposes stable `code()` and `fix_hint()`.
3. **REQ-SAG-0102** — Fixture `tests/fixtures/jev/cross_impl/` (payload-only) + scaffold `tests/fixtures/cross-impl/branches/` (subjects + request filled in **a-3** after atm-core PR #2308); Rust tests match Python reference digests.
4. **NFR-SAG-0001** — `sc-ai-cli` JSON error shape types stubbed (`ScAgentErrorResponse`) for CLI crates.
5. **ADR-SAG-0001** — Error enum is the only source of CLI `error` ids.
6. **ADR-SAG-0002** — Payload bytes use sorted keys, compact JSON, UTF-8 (`BTreeMap` / serde policy documented).
7. **REQ-SAG-0120** — Module `sc-agent-types::observability::v2` (`SessionLog` port; `sc-observability-types = "1.5.0"` on **this crate only** for shim).
8. **REQ-SAG-0121** — `NoopSessionLog` + `test-util` doubles; bead `agt-obs-v2-shim-removal` to delete module + dependency after release 1–2.
9. **ADR-SAG-0016** — No separate observability crate on crates.io; full `sc-observability` only in `sc-ai`.
10. **REQ-SAG-0103** — Freeze `JudgeBackend`, `JudgeRequest`, `PayloadBundle`, `LlmWireContext`, `JudgeOutcome`, `EqualInputRecord`, `BackendId`, `LogEvent`; contract tests + `MockJudgeBackend`.

## Explicit code samples

Authoritative signatures: `docs/sc-agent-types/architecture.md` (same as phase plan).

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0101 | Every README error id is a `JevError` variant with `code()` + `fix_hint()` |
| req:REQ-SAG-0102 | Cross-impl fixture bytes match Python reference SHA-256 |
| req:REQ-SAG-0103 | Contract tests compile against frozen `JudgeBackend` / `JudgeRequest` |
| req:REQ-SAG-0120 | `SessionLog` + `LogEvent` fields defined (no placeholder-only trait) |
| boundary:agent-types | sc-lint manifests stubbed for Phase A boundaries |
| req:REQ-SAG-0100 | Workspace members include all seven Phase A crate paths |

## Required validation

```bash
test -f Cargo.toml && test -f justfile && test -f sc-lint.toml
rg -q 'sc-jev-check' Cargo.toml && rg -q 'sc-jev-py' Cargo.toml
cargo check --workspace
just lint
just test
cargo test -p sc-agent-types -- --nocapture
cargo test -p sc-agent-types jev_cross_impl -- --nocapture
! rg -l 'reqwest|hyper|ureq' crates/ 2>/dev/null | rg -v sc-agent-types || test $(rg -l 'reqwest|hyper|ureq' crates/ 2>/dev/null | wc -l) -eq 0
```

Expected: all commands exit 0; last command confirms no HTTP client deps in workspace yet.

## Parity checklist

Enables future rows; directly supports README tighten **#1**, **#10**.

## This sprint does not close

Check.toml parsing, backends, CLI, PyO3, live Jev.
