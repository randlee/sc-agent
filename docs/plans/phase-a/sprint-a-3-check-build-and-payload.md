---
id: a-3
phase: a
status: planned
closure_type: boundary
target_boundary: jev-check
track: build
wave: 2
branch: sprint/a-3-check-build-and-payload
worktree: sprint/a-3-check-build-and-payload
recommended_agent: lai
recommended_model: gpt-5.6-luna-medium
must_follow: [a-1]
parallel_safe: [a-2, a-3b, a-4, a-5]
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
  - "boundaries/jev-check/**"
  - "tests/fixtures/jev/git-repo/**"
  - "tests/fixtures/cross-impl/branches/**"
  - "docs/sc-jev-check/extraction.md"
---

# Sprint a-3 — check build, extractors, payload

## Outcome

**`sc-jev-check`** loads `check.toml`, runs extractors, builds canonical
**`PayloadBundle`**, validates `INPUT_*` without sending. **No tree-sitter or
grammar dependencies** — branch subjects arrive via **`subjects`** input kind
([`extraction.md`](../../../sc-jev-check/extraction.md), ADR-SAG-0015).

## Deliverables

1. **REQ-SAG-0300** — `Extractor` trait; kinds `file`, `table-column`, **`subjects`**, **`command`**.
2. **REQ-SAG-0311** — `subjects` kind: read/validate `sc/subjects/1`; map to question subjects; copy `producer` into session / equal-input metadata.
3. **REQ-SAG-0305** — `command` stdout must be subjects schema (typical: `sc-treesitter extract …`).
4. **REQ-SAG-0301** — Includes `EXTRACTION_*` / `INPUT_*` paths.
5. **REQ-SAG-0302** — Repo-root via `git rev-parse --show-toplevel`.
6. **REQ-SAG-0303** — Question-set schema `plan-anchoring/questions/1`.
7. **REQ-SAG-0309 (tighten #9)** — `table-column` unescaped pipes only.
8. **REQ-SAG-0503 (parse scope)** — Validate `[llm_judge] system_prompt_file` path in `check.toml`.
9. **`crates/sc-jev-check/Cargo.toml`** — deps on `sc-agent-types`, `sc-agent-secrets`, `sc-jev`, `sc-jev-backends` only (**no** grammar crates).
10. **`assets/jev.toml`** — vendor pin defaults (tighten **#12**).
11. Cross-impl **`tests/fixtures/cross-impl/branches/`** — consumer tests: same committed **`subjects.json`** as Python; request bytes + sha256 gate.

## Explicit code samples

```toml
[inputs.hook]
kind = "file"
path = "skills/atm-bd-lite/scripts/atm-bd-task-hook.py"

[inputs.branches]
kind = "subjects"
path = "out/task-hook.subjects.json"
```

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| boundary:jev-check | Config/input errors typed; subjects + command + file + table-column; no HTTP under `src/extract/**` / `src/payload/**`; cross-impl request digest matches fixture |
| ADR-SAG-0002 | Canonical `PayloadBundle` |
| ADR-SAG-0005 | Single canonical payload per check |
| ADR-SAG-0015 | Consumes subjects artifact; links no parser |
| ADR-SAG-0019 | `system_prompt_file` path validated here |

## Required validation

```bash
just lint
cargo test -p sc-jev-check extractors -- --nocapture
cargo test -p sc-jev-check subjects -- --nocapture
cargo test -p sc-jev-check command_extractor -- --nocapture
cargo test -p sc-jev-check payload -- --nocapture
cargo test -p sc-jev-check cross_impl_branches -- --nocapture
cargo test -p sc-jev-check -- test_payload_refuses_invalid_input --exact --nocapture
cargo test -p sc-jev-check -- test_repo_root_resolution --exact --nocapture
cargo test -p sc-jev-check -- test_question_set_schema --exact --nocapture
! rg -l 'tree-sitter' crates/sc-jev-check/Cargo.toml crates/sc-jev-check/src 2>/dev/null
```

Expected: exit 0; uses committed `subjects.json` from fixture (produced in **a-3b**).

## This sprint does not close

`sc-treesitter` binary (a-3b); HTTP backends; `run_check`; sessions.
