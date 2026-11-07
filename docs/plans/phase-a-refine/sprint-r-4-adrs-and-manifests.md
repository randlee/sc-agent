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
adrs:
  - ADR-SAG-0005
  - ADR-SAG-0016
  - ADR-SAG-0017
  - ADR-SAG-0018
  - ADR-SAG-0023
owned_paths:
  - "Cargo.toml"
  - "crates/sc-ai/Cargo.toml"
  - "crates/sc-ai/src/main.rs"
  - "crates/sc-treesitter/Cargo.toml"
  - "crates/sc-treesitter/src/main.rs"
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
  - "docs/architecture.md#ADR-SAG-0023"
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

**Why wave 1, parallel with r-1:** ADR text plus manifests. r-1 keeps the thin
`operational_io`/`operational_config` constructors, so neither `main.rs`
changes there; r-1 owns no `Cargo.toml` outside `sc-agent-types`. r-4 adds
`clap` as an optional dependency now (unused until r-3c/r-3b) so the wave-2
and wave-4 parser sprints change no manifest that r-3 also edits.

## Closes findings

ARCH-001 (phase-end), RBP-F001 (a-9), ARCH-002 (phase-end), ARCH-006
(phase-end, kinds sentence), ARCHP-005, ARCHP-006, RBQA-F007.

## Deliverables

1. **NFR-SAG-0004 / ADR-SAG-0023.** Root `[workspace.dependencies]`: caret
   ranges for every dependency except deliberate lockstep, which keeps `=` and
   is listed in ADR-SAG-0023: `sc-observability =1.5.0` (v2 shim alignment);
   `tree-sitter` and grammar crates (output determinism, Phase A resolved
   decision 3); intra-workspace `sc-agent-compose` (effective when the crate
   lands in r-3b). `mimalloc = "0.1.52"`; add `clap = { version = "4", features = ["derive"] }`.
   Replace the "pinning policy" comment with a pointer to ADR-SAG-0023. The
   `sc-observability-types` entry stays; r-3 deletes it.
   `.github/scripts/check-exact-pins.sh` lists every `=` in any `Cargo.toml`
   and fails unless it is on the ADR-SAG-0023 list (consumer: CI; prevents
   the a-9 drift ARCH-001 observed; retired never — it is the NFR's test).
2. **Allocator and `cli` feature.** `sc-treesitter`:
   `default = ["cli", "python", "rust"]`; `sc-ai`: `default = ["cli"]`; in
   both `cli = ["dep:mimalloc", "dep:clap"]`,
   `mimalloc = { workspace = true, optional = true }`,
   `clap = { workspace = true, optional = true }`,
   `[[bin]] required-features = ["cli"]`, and
   `#[cfg(feature = "cli")] #[global_allocator]` in each `main.rs`. Delete
   the "cost accepted" comment in `sc-treesitter/Cargo.toml`. Boundary
   manifests are updated by the crates' own sprints (r-3c, r-3b); r-4 adds the
   two `sc-lint.toml` ownership records `mimalloc` and `clap` →
   `crates/sc-ai/Cargo.toml`, `crates/sc-treesitter/Cargo.toml`.
3. **REQ-SAG-0134 (mimalloc) CI.** In `.github/workflows/sc-agent-ci.yml`:
   `cargo build -p sc-treesitter --lib --no-default-features`, the same for
   `sc-ai`, and `.github/scripts/cargo-tree-gates.sh` with the two mimalloc
   rows (`sc-ai`, `sc-treesitter`). Keep a-10's `cargo hack` matrix. The clap
   rows (REQ-SAG-0135) are added by r-3b.
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
7. Flip ADR-SAG-0023 → Active and the r-4 amendment subsections → Active.
   Add ADR-SAG-0023..0027 rows to the ADR index table (status as each lands).

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:NFR-SAG-0004 | `bash .github/scripts/check-exact-pins.sh` passes and fails on an injected unlisted `=` (script self-test `--self-test`) |
| req:REQ-SAG-0134 | CI job present; `--no-default-features` lib builds green; no `mimalloc` in either library tree |
| req:REQ-SAG-0133 | ADR-SAG-0017 table matches the consumer input and the rendered Homebrew formula; bead + upstream issue URL recorded |
| adr:ADR-SAG-0023 | Status Active; the compose-pin bullet reads "effective when the crate lands (r-3b)" |
| adr:ADR-SAG-0017 | r-4 subsection Active |
| adr:ADR-SAG-0016 | r-4 subsection Active |
| adr:ADR-SAG-0005 | r-4 subsection Active; contains no ADR-SAG-0024/`prepare` reference |
| boundary:release | Consumer preflight + publish dry-run green; `sc-lint` boundary output attached |

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
bash .github/scripts/run-consumer-preflight.sh
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-secrets -p sc-jev -p sc-jev-backends -p sc-jev-check -p sc-treesitter -p sc-ai
```

## This sprint does not close

Busy-retry policy (r-3, ADR-SAG-0007, owner decision (b)); the
ADR-SAG-0005 `prepare` sentence (r-2); new-crate release entries (r-3, r-3b);
clap parsers (r-3c, r-3b) and the clap `cargo tree` rows (r-3b); upstream fix
for the Scoop multi-shim limit.
