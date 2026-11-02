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
  - "crates/sc-jev-check/Cargo.toml"
  - "crates/sc-jev-check/src/lib.rs"
  - "boundaries/jev-check/**"
  - "tests/fixtures/jev/git-repo/**"
---

# Sprint a-3 — check build, extractors, payload

## Outcome

**`sc-jev-check`** loads `check.toml`, runs git extractors at repo root, expands
question sets, builds canonical **`PayloadBundle`** bytes, validates `INPUT_*`
without sending. Vendor pin defaults live in packaged `jev.toml` (tighten **#12**).
Parses `[llm_judge] system_prompt_file` (file read at send in a-6).

## Deliverables

1. **REQ-SAG-0300** — Extractors: `file`, `table-column`, **`branches`**, **`command`**.
2. **REQ-SAG-0304** — `branches`: tree-sitter, `language` in check.toml; Phase A
   Python grammar; README `test_python_branches_lists_arms_and_refusals` passes.
3. **REQ-SAG-0301** — Pre-send validation errors before any backend call.
4. **REQ-SAG-0302** — Repo-root resolution via `git rev-parse --show-toplevel`.
5. **REQ-SAG-0303** — Question-set schema `plan-anchoring/questions/1`.
6. **ADR-SAG-0004/0005/0015** — One payload; max bytes; branch extraction policy.
7. Parse **`[llm_judge] system_prompt_file`** (ADR-SAG-0019); default asset path in `jev.toml`.
8. **`src/lib.rs`** declares `pub mod` for `config`, `extract`, `payload`, `session`,
   `run`, `equal_input`, and `registry`. a-3 implements the first three; **`session`,
   `run`, `equal_input`, `registry`** are empty compiling stubs so **a-6** fills
   owned_paths **without editing `lib.rs` again**.

## Explicit code samples

```rust
// crates/sc-jev-check/src/lib.rs (a-3)
pub mod config;
pub mod extract;
pub mod payload;
pub mod session;      // stub until a-6
pub mod run;          // stub until a-6
pub mod equal_input;  // stub until a-6
pub mod registry;     // stub until a-6
```

```toml
[[extractors]]
id = "policy-arms"
type = "branches"
language = "python"
path = "src/policy.py"
rev = "HEAD"
```

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0304 | Python branch fixture matches README arms/refusals test |
| req:REQ-SAG-0301 | Oversized / invalid input refuses before send |
| req:REQ-SAG-0303 | Question-set expansion produces stable canonical bytes |
| boundary:jev-check | `sc-jev-check` crate builds; no HTTP client deps in this crate |

## Required validation

```bash
just lint
cargo test -p sc-jev-check build -- --nocapture
cargo test -p sc-jev-check extractors -- --nocapture
cargo test -p sc-jev-check branches -- --nocapture
cargo test -p sc-jev-check payload -- --nocapture
```

Expected: exit 0; python-branches parity test included.

## This sprint does not close

HTTP backends, `run_check`, sessions, CLI, reading LLM prompt file at send.
