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
parallel_safe: [a-2, a-4, a-5]
requirements:
  - REQ-SAG-0300
  - REQ-SAG-0301
  - REQ-SAG-0302
  - REQ-SAG-0303
  - REQ-SAG-0304
  - REQ-SAG-0305
  - REQ-SAG-0309
adrs:
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
  - "crates/sc-jev-check/assets/queries/**"
  - "crates/sc-jev-check/Cargo.toml"
  - "crates/sc-jev-check/src/lib.rs"
  - "boundaries/jev-check/**"
  - "tests/fixtures/jev/git-repo/**"
  - "tests/fixtures/cross-impl/branches/**"
  - "docs/sc-jev-check/extraction.md"
---

# Sprint a-3 — check build, extractors, payload

## Outcome

**`sc-jev-check`** loads `check.toml`, runs extractors at repo root, builds
canonical **`PayloadBundle`**, validates `INPUT_*` without sending. Extraction
follows **ADR-SAG-0015** / [`extraction.md`](../../../sc-jev-check/extraction.md)
(`.scm` queries — no hard-coded per-language branch logic in Rust).

**Dependency:** atm-core **PR #2308** — Python `python-branches` uses source
slices (not `ast.unparse`) so cross-impl branches fixture can pass.

## Deliverables

1. **REQ-SAG-0300** — `Extractor` trait; kinds `file`, `table-column`, **`tree-sitter`**, **`command`**.
2. **REQ-SAG-0304** — `tree-sitter`: query file path (or inline), captures list, subject rules, pinned grammars; README `test_python_branches_lists_arms_and_refusals` passes via `queries/python-branches.scm`.
3. **REQ-SAG-0305** — `command` extractor contract (argv, JSON stdout, timeouts, session audit fields).
4. **REQ-SAG-0301** — Includes `EXTRACTION_PARSE_ERROR` and existing `INPUT_*` / `EXTRACTION_*` paths.
5. **REQ-SAG-0302** — Repo-root via `git rev-parse --show-toplevel`.
6. **REQ-SAG-0303** — Question-set schema `plan-anchoring/questions/1`.
7. **REQ-SAG-0309 (tighten #9)** — `table-column` splits on **unescaped** pipes only.
8. Parse **`[llm_judge] system_prompt_file`** (file read at send in a-6).
9. **`src/lib.rs`** declares `session`, `run`, `equal_input`, `registry` as compiling stubs for a-6.
10. Cross-impl **`tests/fixtures/cross-impl/branches/`** — subjects JSON + full request sha256 (Python + Rust).

## Explicit code samples

```toml
[[extractors]]
id = "branches"
kind = "tree-sitter"
language = "python"
path = "skills/atm-bd-lite/scripts/atm-bd-task-hook.py"
rev = "HEAD"
query = "queries/python-branches.scm"
captures = ["arm", "refusal"]
```

See committed query: `tests/fixtures/cross-impl/branches/queries/python-branches.scm`.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0304 | Seven named tree-sitter tests + README python-branches row |
| req:REQ-SAG-0305 | Command extractor typed errors on bad JSON / exit / timeout |
| req:REQ-SAG-0309 | Escaped `\|` in table-column fixtures |
| boundary:jev-check | No HTTP deps; queries are config, not Rust match logic |

## Required validation

```bash
just lint
cargo test -p sc-jev-check extractors -- --nocapture
cargo test -p sc-jev-check tree_sitter -- --nocapture
cargo test -p sc-jev-check command_extractor -- --nocapture
cargo test -p sc-jev-check payload -- --nocapture
cargo test -p sc-jev-check cross_impl_branches -- --nocapture
```

Expected: exit 0; branches cross-impl matches Python reference after PR #2308.

## This sprint does not close

HTTP backends, `run_check`, sessions, CLI prompt file read at send.
