---
id: a-3b
phase: a
status: planned
closure_type: boundary
target_boundary: treesitter
track: build
wave: 2
branch: sprint/a-3b-sc-treesitter
worktree: sprint/a-3b-sc-treesitter
recommended_agent: lai
recommended_model: gpt-5.6-luna-medium
must_follow: [a-1]
parallel_safe: [a-2, a-3, a-4, a-5]
requirements:
  - REQ-SAG-0310
  - REQ-SAG-0101
adrs:
  - ADR-SAG-0015
owned_paths:
  - "crates/sc-treesitter/**"
  - "boundaries/treesitter/**"
  - "docs/sc-treesitter/**"
---

# Sprint a-3b — `sc-treesitter` producer

## Outcome

New crate + binary **`sc-treesitter`**: pinned grammars, query assets, CLI and library
that emit **`sc/subjects/1`** JSON. No Jev or `sc-jev-check` dependency.

## Deliverables

1. **REQ-SAG-0310** — Workspace member `sc-treesitter`; feature-flagged grammar deps; assets `queries/python/branches.scm`, `queries/rust/match-arms.scm`; CLI `extract` / `languages` / `queries`; library target; git-rev reads + `blob_sha`; typed errors per architecture doc.
2. Tests: six former `test_tree_sitter_*` cases, `test_matches_mode_duplicate_captures`, `test_subjects_schema_roundtrip`.
3. Producer fixture: regenerate committed `tests/fixtures/cross-impl/branches/subjects.json` from hook + query (byte-for-byte stability test in this crate).
4. Migrate lite-gate query from `tests/fixtures/cross-impl/branches/queries/python-branches.scm` into crate assets (keep fixture copy or symlink policy documented in test).

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| boundary:treesitter | No dependency on `sc-jev-check` / `sc-jev`; stdout subjects validate `sc/subjects/1` |
| ADR-SAG-0015 | Producer only; static pinned grammars; `producer.mode = matches` |

## Required validation

```bash
just lint
cargo test -p sc-treesitter -- --nocapture
sc-treesitter languages
sc-treesitter extract --language python --query branches --rev HEAD tests/fixtures/cross-impl/branches/hook.py 2>/dev/null | head -1
```

Expected: tests pass; extract emits JSON with `"schema":"sc/subjects/1"`.

## This sprint does not close

`sc-jev-check` `subjects` input kind (a-3); cross-impl consumer digest test (a-3).
