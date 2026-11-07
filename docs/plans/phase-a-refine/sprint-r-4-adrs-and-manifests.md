---
id: r-4
phase: r
status: planned
closure_type: contract
target_boundary: release
track: release
wave: 1
size: M
branch: sprint/r-4-adrs-and-manifests
worktree: sprint/r-4-adrs-and-manifests
recommended_agent: sai
recommended_model: gpt-5.6-sol-medium
must_follow: [a-10]
parallel_safe: [r-1]
requirements:
  - REQ-SAG-0133
  - REQ-SAG-0134
  - NFR-SAG-0004
  - REQ-SAG-0130
  - REQ-SAG-0131
  - REQ-SAG-0132
  - REQ-SAG-0310
  - REQ-SAG-0703
adrs:
  - ADR-SAG-0005
  - ADR-SAG-0010
  - ADR-SAG-0015
  - ADR-SAG-0016
  - ADR-SAG-0017
  - ADR-SAG-0018
  - ADR-SAG-0023
owned_paths:
  - "Cargo.toml (all but the sc-observability-types line, which r-1 deletes)"
  - "crates/sc-ai/Cargo.toml"
  - "crates/sc-treesitter/Cargo.toml"
  - "release/**"
  - ".github/workflows/*-publish.yml"
  - ".github/workflows/release*.yml"
  - ".github/workflows/prerelease-archive.yml"
  - ".github/workflows/sc-publish-dry-run.yml"
  - ".github/workflows/sc-agent-ci.yml"
  - ".github/scripts/cargo-tree-gates.sh"
  - ".github/scripts/check-exact-pins.sh"
  - "sc-lint.toml"
  - "docs/development/sc-publish.md"
  - "docs/architecture.md#ADR-SAG-0005 amendment (r-4)"
  - "docs/architecture.md#ADR-SAG-0016 amendment (r-4)"
  - "docs/architecture.md#ADR-SAG-0017 amendment (r-4)"
  - "docs/architecture.md#ADR-SAG-0023 (Status line and r-4 bullets)"
  - "docs/architecture.md#ADR index row ADR-SAG-0023"
  - "docs/requirements.md#REQ-SAG-0133, REQ-SAG-0134, NFR-SAG-0004 rows"
---

# Sprint r-4 — pins, allocator, released binaries (D8, D13 kinds sentence)

## Outcome

The pin, allocator and second-binary decisions have an ADR (ADR-SAG-0023) and
the manifests match it. Library crates use caret ranges, mimalloc and clap are
optional deps of the two binary crates behind the default `cli` feature, the
CI workflow proves the library builds without mimalloc, and ADR-SAG-0017 names
both released binaries with per-channel coverage. ADR-SAG-0016's version text
and ADR-SAG-0005's extractor-kinds sentence are corrected.

**Why wave 1, parallel with r-1:** ADR text plus manifests. r-4 edits no
`main.rs`: `[[bin]] required-features = ["cli"]` compiles each binary only
when `cli` (and so `dep:mimalloc`) is enabled, so the existing unconditional
`#[global_allocator]` stays valid; both `main.rs` files belong to r-1 in
wave 1. r-1 owns no `Cargo.toml` outside `sc-agent-types`, except deleting the
root `sc-observability-types` line. **Merge order:** r-1 merges first; r-4
rebases onto it before its final validation, because `check-exact-pins.sh`
fails on the `sc-observability-types = "=1.5.0"` entry, which is not on the
ADR-SAG-0023 list and which r-1 deletes. r-4 adds `clap` as an optional
dependency now (unused until r-3c/r-3b) so the wave-2 and wave-4 parser
sprints change no manifest that r-3 also edits.

## Closes findings

ARCH-001 (phase-end), RBP-F001 (a-9), ARCH-002 (phase-end), ARCH-006
(phase-end, kinds sentence), ARCHP-005, ARCHP-006, RBQA-F007.

## Deliverables

1. **NFR-SAG-0004 / ADR-SAG-0023.** Root `[workspace.dependencies]`: caret
   ranges for every dependency except deliberate lockstep, which keeps `=`.
   **One authoritative lockstep list** lives in ADR-SAG-0023 between the
   markers `<!-- lockstep-pins:start -->` and `<!-- lockstep-pins:end -->`,
   one crate name per line: `sc-observability` (v2 shim alignment),
   `tree-sitter`, `tree-sitter-python`, `tree-sitter-rust` (output
   determinism, Phase A resolved decision 3), `sc-agent-compose`
   (intra-workspace lockstep; in force from r-3b). `sc-observability-types`
   is **not** on the list: r-1 removes its only user and its root entry.
   `mimalloc = "0.1.52"`; add `clap = { version = "4", features = ["derive"] }`.
   Replace the "pinning policy" comment with a pointer to ADR-SAG-0023.
   `.github/scripts/check-exact-pins.sh` reads the names between those
   markers in `docs/architecture.md` (no second copy of the list in the
   script), lists every `=` requirement in any `Cargo.toml`, and fails on any
   name not on the list (consumer: CI; prevents the a-9 drift ARCH-001
   observed; retired never — it is the NFR's test). `--self-test` injects an
   unlisted `=` and expects failure. r-4 deletes the `sc-lint.toml`
   `sc-observability-types` ownership record (no user remains after r-1).
2. **Allocator and `cli` feature.** `sc-treesitter`:
   `default = ["cli", "python", "rust"]`; `sc-ai`: `default = ["cli"]`; in
   both `cli = ["dep:mimalloc", "dep:clap"]`,
   `mimalloc = { workspace = true, optional = true }`,
   `clap = { workspace = true, optional = true }`, and
   `[[bin]] required-features = ["cli"]`. No `main.rs` edit: with the bin
   gated on `cli`, the existing unconditional `#[global_allocator]` and
   `use mimalloc::MiMalloc` compile only when `dep:mimalloc` is present, and
   `cargo hack --feature-powerset` skips the bin for feature sets without
   `cli`. Delete the "cost accepted" comment in `sc-treesitter/Cargo.toml`. Boundary
   manifests are updated by the crates' own sprints (r-3c, r-3b); r-4 adds the
   two `sc-lint.toml` ownership records `mimalloc` and `clap` →
   `crates/sc-ai/Cargo.toml`, `crates/sc-treesitter/Cargo.toml`.
3. **REQ-SAG-0134 (mimalloc) CI.** In `.github/workflows/sc-agent-ci.yml`:
   `cargo build -p sc-treesitter --lib --no-default-features`, the same for
   `sc-ai`, and `.github/scripts/cargo-tree-gates.sh` with the two mimalloc
   rows (`sc-ai`, `sc-treesitter`; word-bounded pattern, plan "`cargo tree`
   gates"). Keep a-10's `cargo hack` matrix and add
   `cargo hack check -p sc-agent-types --feature-powerset --no-dev-deps`
   (r-1's feature seams; valid on the a-10 feature set too). The clap rows
   (REQ-SAG-0135) and the remaining rows are added by r-3b.
4. **REQ-SAG-0133 / ADR-SAG-0017 r-4 amendment.** Both `sc-ai` and
   `sc-treesitter` are released binaries:

   | Channel | sc-ai | sc-treesitter |
   | --- | --- | --- |
   | GitHub Release archive | yes | yes |
   | crates.io (`cargo install`) | yes | yes |
   | Homebrew | yes | yes (same formula) |
   | Scoop | shim | in archive, no shim |
   | WinGet | alias | in package, no alias |

   The Scoop/WinGet single-shim limit is an sc-publish kit limitation: file
   bead `agt-scoop-multi-shim` and an upstream sc-publish issue, record its URL
   in the amendment. `docs/development/sc-publish.md` documents the Windows
   invocation path. If a kit-supported field for extra binaries exists, set it
   in the consumer input and re-run `install.py` (r-4 owns the rendered files
   this wave).
5. **ADR-SAG-0016 r-4 amendment:** version text `"1.5.0"` → `=1.5.0`
   (exact, lockstep per ADR-SAG-0023).
6. **ADR-SAG-0005 r-4 amendment (D13):** "`sc-jev-check` runs the four
   consumer extractor kinds (`file`, `table-column`, `subjects`, `command`);
   it is not the only producer of subjects (ADR-SAG-0015)." No reference to
   `prepare` or ADR-SAG-0024 (r-2 lands that sentence).
7. Flip ADR-SAG-0023's Status line and r-4 bullets → Active (its r-3b
   subsection stays Proposed) and the r-4 amendment subsections → Active.
   The ADR index rows for ADR-SAG-0023..0028 already exist (written by the
   plan PR); r-4 edits only the ADR-SAG-0023 row's Status cell.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:NFR-SAG-0004 | `bash .github/scripts/check-exact-pins.sh` passes and fails on an injected unlisted `=` (script self-test `--self-test`) |
| req:REQ-SAG-0134 | CI job present; `--no-default-features` lib builds green; no `mimalloc` in either library tree |
| req:REQ-SAG-0133 | ADR-SAG-0017 table matches the consumer input and the rendered Homebrew formula; bead + upstream issue URL recorded |
| adr:ADR-SAG-0023 | Status Active; the lockstep list between the markers is the one `check-exact-pins.sh` reads (no list in the script); `sc-observability-types` absent; the r-3b subsection stays Proposed |
| adr:ADR-SAG-0017 | r-4 subsection Active |
| adr:ADR-SAG-0016 | r-4 subsection Active |
| adr:ADR-SAG-0005 | r-4 subsection Active; contains no ADR-SAG-0024/`prepare` reference |
| boundary:release | Consumer preflight + publish dry-run green; `sc-lint` boundary output attached. Regression: REQ-SAG-0310 `cargo test -p sc-treesitter --test cli` (the `cli`-gated binary still builds and runs); REQ-SAG-0703 install path in `docs/development/sc-publish.md` names both binaries; ADR-SAG-0010 `cargo test -p sc-ai --test cli` |

## Required validation

```bash
just lint
just test
cargo build -p sc-treesitter --lib --no-default-features
cargo build -p sc-ai --lib --no-default-features
bash .github/scripts/cargo-tree-gates.sh
bash .github/scripts/check-exact-pins.sh
bash .github/scripts/check-exact-pins.sh --self-test
cargo hack check -p sc-treesitter --feature-powerset --no-dev-deps
cargo hack check -p sc-agent-types --feature-powerset --no-dev-deps
cargo test -p sc-treesitter --test cli
cargo test -p sc-ai --test cli
git diff --quiet origin/integrate/phase-a -- crates/sc-ai/src/main.rs crates/sc-treesitter/src/main.rs
bash .github/scripts/run-consumer-preflight.sh
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-secrets -p sc-jev -p sc-jev-backends -p sc-jev-check -p sc-treesitter -p sc-ai
```

## This sprint does not close

Busy-retry policy (r-3, ADR-SAG-0007, owner decision (b)); the
ADR-SAG-0005 `prepare` sentence (r-2); new-crate release entries (r-3, r-3b);
clap parsers (r-3c, r-3b) and the clap `cargo tree` rows (r-3b); upstream fix
for the Scoop multi-shim limit.
