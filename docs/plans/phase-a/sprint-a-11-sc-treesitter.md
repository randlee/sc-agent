---
id: a-11
phase: a
status: complete
closure_type: boundary
target_boundary: treesitter
track: build
wave: 2
branch: sprint/a-11-sc-treesitter
worktree: sprint/a-11-sc-treesitter
recommended_agent: lai
recommended_model: gpt-5.6-luna-medium
must_follow: [a-1]
parallel_safe: [a-2, a-3, a-4, a-5]
requirements:
  - REQ-SAG-0310
  - REQ-SAG-0101
  - REQ-SAG-0104
  - NFR-SAG-0001
adrs:
  - ADR-SAG-0001
  - ADR-SAG-0015
owned_paths:
  - "crates/sc-treesitter/**"
  - "boundaries/sc-treesitter/**"
  - "docs/sc-treesitter/**"
  - "tests/fixtures/cross-impl/branches/**"
---

# Sprint a-11 — `sc-treesitter` producer

## Outcome

Crate + binary **`sc-treesitter`**: pinned grammars, query assets, CLI and library
emitting **`sc/subjects/1`** JSON. Sole owner of the **branches cross-impl producer
fixture** (`tests/fixtures/cross-impl/branches/**`). No Jev or `sc-jev-check` dependency.

## Deliverables

1. **REQ-SAG-0310** — Workspace member; feature-flagged grammars; assets
   `queries/python/branches.scm`, `queries/rust/match-arms.scm`; CLI `extract` /
   `languages` / `queries` (always **`matches`** mode; optional `--captures` is an
   allow-list only); library target; `git show` reads + `blob_sha`.
2. **REQ-SAG-0101** — CLI stderr uses **`JevError`** ids (including
   `LANGUAGE_UNAVAILABLE`, `QUERY_*`, `INPUT_MISSING`; shared extraction ids).
3. Named tests (each locks one schema rule):
   - `test_tree_sitter_python_branches_lists_arms_and_refusals`
   - `test_tree_sitter_rust_match_arms`
   - `test_tree_sitter_helper_capture_is_not_a_subject`
   - `test_tree_sitter_same_line_ids_are_unique_and_stable`
   - `test_tree_sitter_parse_error_is_typed`
   - `test_tree_sitter_unknown_language_is_typed` → `LANGUAGE_UNAVAILABLE`
   - `test_matches_mode_duplicate_captures`
   - `test_subjects_schema_roundtrip`
4. Fixture: author `hook.py`, `subjects.json`, and expected request artifacts;
   crate test asserts `cargo run -p sc-treesitter -- extract …` stdout equals committed
   `subjects.json` byte-for-byte.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| boundary:treesitter | No `sc-jev-check` / `sc-jev` dep; stdout validates `sc/subjects/1` |
| ADR-SAG-0001 | All CLI errors are `JevError` codes from contract snapshot |
| ADR-SAG-0015 | Static pinned grammars; `producer.mode = "matches"` always |

## Required validation

```bash
set -o pipefail
just lint
cargo test -p sc-treesitter -- --nocapture
cargo run -p sc-treesitter -- languages
cargo run -p sc-treesitter -- extract --language python --query branches --rev HEAD \
  tests/fixtures/cross-impl/branches/hook.py | python3 -c "import json,sys; assert json.load(sys.stdin)['schema']=='sc/subjects/1'"
```

Expected: tests pass; extract JSON has `"schema":"sc/subjects/1"`.

## This sprint does not close

`sc-jev-check` `subjects` input (a-3); end-to-end CLI cross-impl digest (a-7).
