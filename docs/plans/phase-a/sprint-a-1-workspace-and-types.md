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
  - REQ-SAG-0005
  - REQ-SAG-0100
  - REQ-SAG-0101
  - REQ-SAG-0102
  - REQ-SAG-0103
  - REQ-SAG-0104
  - REQ-SAG-0120
  - REQ-SAG-0121
  - NFR-SAG-0001
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0002
  - ADR-SAG-0006
  - ADR-SAG-0016
  - ADR-SAG-0019
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
  - "crates/sc-agent-secrets/Cargo.toml"
  - "crates/sc-agent-secrets/src/lib.rs"
  - "crates/sc-jev/Cargo.toml"
  - "crates/sc-jev/src/lib.rs"
  - "crates/sc-jev-backends/Cargo.toml"
  - "crates/sc-jev-backends/src/lib.rs"
  - "crates/sc-jev-check/Cargo.toml"
  - "crates/sc-jev-check/src/lib.rs"
  - "crates/sc-ai/Cargo.toml"
  - "crates/sc-ai/src/main.rs"
  - "crates/sc-jev-py/Cargo.toml"
  - "crates/sc-jev-py/src/lib.rs"
  - "crates/sc-treesitter/Cargo.toml"
  - "crates/sc-treesitter/src/lib.rs"
  - "docs/sc-agent-types/subjects-schema.md"
  - "tests/fixtures/jev/subjects-schema/**"
---

# Sprint a-1 — workspace, sc-lint, errors, cross-impl fixture

## Outcome

Rust workspace exists with sc-lint boundaries, `sc-agent-types` error enum
(with fix text per variant), and a **cross-implementation** payload fixture
verified against committed Python reference bytes. **No network code.**

## Deliverables

1. **REQ-SAG-0100** — Root **`Cargo.toml` `[workspace.members]`** lists every Phase A crate:
   `sc-agent-types`, `sc-agent-secrets`, `sc-jev`, `sc-jev-backends`, `sc-jev-check`,
   `sc-treesitter`, `sc-ai`, `sc-jev-py`. a-1 commits **minimal compiling stubs** (`Cargo.toml` + root
   module only) for each member except `sc-agent-types` (fully implemented here).
   Later sprints **replace stub bodies** without editing root `Cargo.toml` members list.
   **`sc-jev-check/src/lib.rs`** — a-1 commits the **full `mod` tree** and empty
   module files (`config`, `extract`, `payload`, `session`, `run`, `equal_input`,
   `registry`); a-3 and a-6 fill bodies only, never restructure `lib.rs`.
   `just lint` / `just test`, sc-lint manifests for Phase A boundaries.
2. **REQ-SAG-0101** — `JevError` enum covering every id in `tests/fixtures/jev/README-contract/errors.json`; each variant exposes stable `code()` and `fix_hint()`.
3. **REQ-SAG-0102** — Fixture `tests/fixtures/jev/cross_impl/` (payload-only); Rust workspace test `tests/jev_cross_impl/` matches Python reference digests (not `cargo test -p sc-agent-types` alone).
4. **REQ-SAG-0005** — Workspace layout matches `docs/architecture.md` and per-crate requirement tables.
5. **NFR-SAG-0001** — `sc-ai-cli` JSON error shape types stubbed (`ScAgentErrorResponse`) for CLI crates.
6. **ADR-SAG-0001** — Error enum is the only source of CLI `error` ids.
7. **ADR-SAG-0002** — Payload bytes use sorted keys, compact JSON, UTF-8 (`BTreeMap` / serde policy documented).
8. **REQ-SAG-0120** — Module `sc-agent-types::observability::v2` (`SessionLog` port; `sc-observability-types = "1.5.0"` on **this crate only** for shim).
9. **REQ-SAG-0121** — `NoopSessionLog` + `test-util` doubles; bead `agt-obs-v2-shim-removal` to delete module + dependency after release 1–2.
10. **ADR-SAG-0016** — No separate observability crate on crates.io; full `sc-observability` on composition roots **`sc-ai`** and **`sc-jev-py`** (a-8 wires PyO3 logger).
11. **REQ-SAG-0104** — Rust types + JSON fixture for **`sc/subjects/1`** ([`subjects-schema.md`](../../../sc-agent-types/subjects-schema.md)); commit **`tests/fixtures/jev/subjects-schema/v1/minimal.json`** (valid minimal document for a-3 unit tests); producer/consumer pin same schema id.
12. **REQ-SAG-0103** — Freeze `JudgeBackend`, `JudgeRequest`, `PayloadBundle`, `LlmWireContext`, `JudgeOutcome`, `EqualInputRecord`, `BackendId`, `LogEvent`, `RepoRoot`, `Rev`, `SessionId`, `Sha256Digest`, extractor domain types; contract tests + `MockJudgeBackend`.

## Explicit code samples

Authoritative signatures: `docs/sc-agent-types/architecture.md` (same as phase plan).

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| boundary:agent-types | sc-lint manifests stubbed under `boundaries/**` for all eight Phase A boundaries |
| ADR-SAG-0001 | `JevError` is the only CLI/PyO3 error id source |
| ADR-SAG-0002 | Canonical payload serde policy documented in crate docs |
| ADR-SAG-0016 | Shim module present; no full `sc-observability` on library crates |

## Required validation

```bash
test -f Cargo.toml && test -f justfile && test -f sc-lint.toml
rg -q 'sc-jev-check' Cargo.toml && rg -q 'sc-treesitter' Cargo.toml && rg -q 'sc-jev-py' Cargo.toml
cargo check --workspace
just lint
just test
cargo test -p sc-agent-types -- --nocapture
cargo test jev_cross_impl -- --nocapture
test -f tests/fixtures/jev/README-contract/errors.json
test -f tests/fixtures/jev/subjects-schema/v1/minimal.json
python3 -c "import json; d=json.load(open('tests/fixtures/jev/subjects-schema/v1/minimal.json')); assert d.get('schema')=='sc/subjects/1'"
python3 -c "import json; json.load(open('tests/fixtures/jev/README-contract/errors.json'))"
! rg -l 'reqwest|hyper|ureq' crates/ 2>/dev/null | rg -v sc-agent-types || test $(rg -l 'reqwest|hyper|ureq' crates/ 2>/dev/null | wc -l) -eq 0
```

Expected: all commands exit 0; last command confirms no HTTP client deps in workspace yet.

## Parity checklist

Enables future rows; directly supports README tighten **#1**, **#10**.

## This sprint does not close

Check.toml parsing, backends, CLI, PyO3, live Jev.
