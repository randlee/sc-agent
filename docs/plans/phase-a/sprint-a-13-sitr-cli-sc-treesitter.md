---
id: a-13
phase: a
status: planned
closure_type: integration
target_boundary: publish-names
track: publish
wave: 8
branch: sprint/a-13-sitr-cli-sc-treesitter
must_follow: [a-12]
parallel_safe: []
requirements:
  - REQ-SAG-0130
  - REQ-SAG-0131
adrs:
  - ADR-SAG-0005
owned_paths:
  - "crates/sitr/"
  - "crates/sc-treesitter/"
  - "crates/sc-treesitter-py/"
  - "crates/sc-agent-py/"
  - "release/sc-publish-consumer-input.json"
  - "release/publish-artifacts.toml"
---

# Sprint a-13 — sitr CLI crate + product names on PyPI

## Outcome

Match the **decidr** product shape for **sitr**:

- **`sc-treesitter`** (crates.io): library only — grammars, queries, `run_query`, PyO3 types behind `python` feature. **No** `[[bin]]`, no clap, no general log in the library.
- **`sitr`** (crates.io): thin CLI crate — `[[bin]] sitr`, clap, `sc-observability` general log; depends on `sc-treesitter`.
- **PyPI `sitr`** wheel: `sc-treesitter-py` / `pyproject.toml` project name **`sitr`**; import module **`sc_treesitter`**. API must cover CLI capabilities (languages, queries, predefined query + file path, captures filter, JSON matching CLI output where practical).
- **PyPI `decidr`** wheel: `sc-agent-py` project name **`decidr`**; import **`sc_agent`** (unchanged). Align consumer input; production PyPI name collision tracked in **agt-8wk**.

## Background

The library and CLI were combined in one package (`sc-treesitter` + `cli` feature) for convenience. Product names **decidr** / **sitr** must be squatted on registries without publishing as **`sc-agent`** / **`sc-treesitter`** on PyPI.

## Deliverables

1. New `crates/sitr/` with `main.rs`, `cli.rs` moved from `crates/sc-treesitter` (or equivalent thin wiring).
2. `crates/sc-treesitter`: revert to package name **`sc-treesitter`**; drop `[[bin]]` and `cli` feature deps from the library crate; keep `python` feature for PyO3.
3. `crates/sc-treesitter-py`: path dependency **`sc-treesitter`** only.
4. Workspace `members` + `Cargo.lock`; CI (`-p sc-treesitter`, `-p sitr`), `scripts/generate-cli-docs.sh`.
5. `release/sc-publish-consumer-input.json`: crates.io **`sc-treesitter`** then **`sitr`** (CLI) in publish order; PyPI **`decidr`** / **`sitr`**; re-run kit `install.py` → `publish-artifacts.toml`.
6. REQ-SAG-0130 acceptance text: product PyPI names **decidr** / **sitr**.

## Acceptance criteria

- [ ] `cargo test -p sc-treesitter --all-features` and `cargo test -p sitr --all-features` pass.
- [ ] `cargo run -p sitr -- languages` matches prior `sitr` CLI behavior.
- [ ] `maturin build` for both py projects; CI pip install **`decidr`** / **`sitr`** wheels.
- [ ] `validate-manifest` + `sc-publish-dry-run.sh` checks pass.
- [ ] `cargo publish --dry-run` includes **`sitr`** as its own package after **`sc-treesitter`**.

## This sprint does not close

- PEP 541 / production PyPI **`decidr`** (agt-8wk).
- Python typed structs (v2 ergonomics).

## Implementation notes

- Prefer moving CLI sources rather than retyping; export any shared helpers from `sc_treesitter` if needed.
- Homebrew/Scoop release binaries remain **`decidr`** + **`sitr`** in archives (unchanged).
