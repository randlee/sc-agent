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
parallel_safe: [a-2, a-3]
requirements:
  - REQ-SAG-0100
  - REQ-SAG-0101
  - REQ-SAG-0102
  - REQ-SAG-0120
  - REQ-SAG-0121
  - NFR-SAG-0001
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0002
  - ADR-SAG-0016
owned_paths:
  - "Cargo.toml"
  - "Cargo.lock"
  - "justfile"
  - "sc-lint.toml"
  - "boundaries/**"
  - "crates/sc-agent-types/**"
  - "crates/sc-agent-types/src/observability/**"
  - "tests/fixtures/jev/**"
  - "tests/jev_cross_impl/**"
---

# Sprint a-1 — workspace, sc-lint, errors, cross-impl fixture

## Outcome

Rust workspace exists with sc-lint boundaries, `sc-agent-types` error enum
(with fix text per variant), and a **cross-implementation** payload fixture
verified against committed Python reference bytes. **No network code.**

## Deliverables

1. **REQ-SAG-0100** — Root workspace, `just lint` / `just test`, sc-lint manifests for Phase A boundaries.
2. **REQ-SAG-0101** — `JevError` enum covering every id in jev-check README Errors table; each variant exposes stable `code()` and `fix_hint()`.
3. **REQ-SAG-0102** — Fixture `tests/fixtures/jev/cross_impl/request.json` + `expected.sha256` + `expected.body.bin` generated from Python reference; Rust test computes same digest.
4. **NFR-SAG-0001** — `sc-ai-cli` JSON error shape types stubbed (`ScAgentErrorResponse`) for CLI crates.
5. **ADR-SAG-0001** — Error enum is the only source of CLI `error` ids.
6. **ADR-SAG-0002** — Payload bytes use sorted keys, compact JSON, UTF-8 (`BTreeMap` / serde policy documented).
7. **REQ-SAG-0120** — Module `sc-agent-types::observability::v2` (`SessionLog` port; `sc-observability-types = "1.5.0"` on **this crate only** for shim).
8. **REQ-SAG-0121** — `NoopSessionLog` + `test-util` doubles; bead `agt-obs-v2-shim-removal` to delete module + dependency after release 1–2.
9. **ADR-SAG-0016** — No separate observability crate on crates.io; full `sc-observability` only in `sc-ai`.

## Acceptance criteria

```bash
test -f Cargo.toml && test -f justfile && test -f sc-lint.toml
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
