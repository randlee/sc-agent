---
id: a-3
phase: a
status: complete
closure_type: boundary
target_boundary: jev-check
track: build
wave: 2
branch: sprint/a-3-check-build-and-payload
worktree: sprint/a-3-check-build-and-payload
recommended_agent: lai
recommended_model: gpt-5.6-luna-medium
must_follow: [a-1]
parallel_safe: [a-2, a-11, a-4, a-5]
requirements:
  - REQ-SAG-0101
  - REQ-SAG-0102
  - REQ-SAG-0300
  - REQ-SAG-0301
  - REQ-SAG-0302
  - REQ-SAG-0303
  - REQ-SAG-0305
  - REQ-SAG-0309
  - REQ-SAG-0311
  - REQ-SAG-0503
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0002
  - ADR-SAG-0004
  - ADR-SAG-0005
  - ADR-SAG-0015
  - ADR-SAG-0019
owned_paths:
  - "crates/sc-jev-check/src/config/**"
  - "crates/sc-jev-check/src/extract/**"
  - "crates/sc-jev-check/src/payload/**"
  - "crates/sc-jev-check/assets/jev.toml"
  - "crates/sc-jev-check/assets/llm-evaluator-system.md"
  - "crates/sc-jev-check/Cargo.toml"
  - "boundaries/sc-jev-check/**"
  - "tests/fixtures/jev/git-repo/**"
  - "docs/sc-jev-check/extraction.md"
---

# Sprint a-3 — check build, extractors, payload

## Outcome

**`sc-jev-check`** loads `check.toml`, runs extractors, builds **`PayloadBundle`**, validates
`INPUT_*` without sending. **No tree-sitter / grammar deps** (ADR-SAG-0015).

## Deliverables

1. **REQ-SAG-0300** — `Extractor` trait (sealed); kinds `file`, `table-column`, `subjects`, `command`.
2. **REQ-SAG-0311** — `subjects` kind validates `sc/subjects/1`; unit tests use **a-1**
   `tests/fixtures/jev/subjects-schema/v1/minimal.json` only.
3. **REQ-SAG-0305** — `command` contract per [`extraction.md`](../../../sc-jev-check/extraction.md).
4. **REQ-SAG-0301** — Refuse backend send when payload over cap (`INPUT_TOO_LARGE`); unit tests on builder only.
5. **REQ-SAG-0302** — Git-relative paths resolve via injected `RepoRoot`; tests with fixture repo under `tests/fixtures/jev/git-repo/`.
6. **REQ-SAG-0303** — Question-set / state JSON schema validation before payload merge.
7. **REQ-SAG-0309** — `table-column` escapes markdown pipe cells per README.
8. **REQ-SAG-0503** — Load LLM system prompt asset paths from check config (no send in a-3).
9. **`Cargo.toml`** — no grammar crates; no HTTP under `src/extract/**` / `src/payload/**`.
10. **`assets/jev.toml`** — vendor pins (tighten #12).

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| boundary:jev-check | All four kinds + payload; sealed trait; no tree-sitter dep |
| ADR-SAG-0015 | Consumes subjects artifact only |

## Required validation

```bash
set -o pipefail
just lint
cargo test -p sc-jev-check extractors -- --nocapture
cargo test -p sc-jev-check subjects -- --nocapture
cargo test -p sc-jev-check command_extractor -- --nocapture
cargo test -p sc-jev-check payload -- --nocapture
! rg -l 'tree-sitter' crates/sc-jev-check/Cargo.toml crates/sc-jev-check/src 2>/dev/null
```

## This sprint does not close

Branches cross-impl digest (a-7 + a-11 fixture); `run_check` (a-6).
