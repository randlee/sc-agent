---
id: r-4
phase: r
status: planned
closure_type: contract
target_boundary: release
track: release
wave: 1
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
  - REQ-SAG-0505
adrs:
  - ADR-SAG-0005
  - ADR-SAG-0007
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
  - "release/sc-publish-consumer-input.json"
  - "docs/development/sc-publish.md"
  - "docs/architecture.md"
  - "docs/requirements.md"
  - ".github/workflows/ci.yml"
---

# Sprint r-4 — pins, allocator, released binaries, ADR wording (D8, D13 wording)

## Outcome

The dependency pin, allocator and second-binary decisions have an ADR
(ADR-SAG-0023) and the manifests match it. Library crates use caret ranges,
mimalloc is linked only by the binaries, and ADR-SAG-0017 names both released
binaries with per-channel coverage. ADR-SAG-0016's version text and
ADR-SAG-0005's scope are corrected, and ADR-SAG-0007 states the retry numbers.

**Why wave 1, parallel with r-1:** this sprint is ADR text plus manifests. Its
paths are disjoint from r-1's (r-1 excludes both `main.rs` files and the root
`Cargo.toml`), and it settles the root `Cargo.toml` before r-3 and r-3b add
members. New-crate release entries and boundaries are **not** here; the
sprints that create those crates own them, because a preflight that lists a
missing Cargo.toml fails.

## Closes findings

ARCH-001 (blocking), ARCH-002, RBP-F001 (a-9), ARCHP-005, ARCHP-006,
RBQA-F007, ARCH-006, a-10 wave-1 follow-up "record retry numbers".

## Deliverables

1. **NFR-SAG-0004 / ADR-SAG-0023.** Root `[workspace.dependencies]`: caret
   ranges for every dependency except deliberate lockstep, which keeps `=` and
   is named in ADR-SAG-0023:
   - `sc-observability =1.5.0` (v2 shim alignment)
   - `tree-sitter` and grammar crates (`sc-treesitter/Cargo.toml`), pinned for
     output determinism per Phase A resolved decision 3
   - intra-workspace `sc-agent-compose` (added in r-3b).

   `mimalloc` becomes `"0.1.52"` (caret). Remove the "pinning policy" comment
   at `Cargo.toml:35` and point to ADR-SAG-0023 instead. Leave the
   `sc-observability-types` entry in place; r-3 deletes it.
2. **Allocator.** `sc-treesitter` and `sc-ai`:
   `[features] default = ["cli"]; cli = ["dep:mimalloc"]`, `mimalloc = { workspace = true, optional = true }`,
   `[[bin]] required-features = ["cli"]`, and
   `#[cfg(feature = "cli")] #[global_allocator]` in each `main.rs`. Delete
   the "cost accepted" comment at `sc-treesitter/Cargo.toml:25`.
3. **REQ-SAG-0134, CI feature matrix** (in the a-10 workflow
   `.github/workflows/ci.yml`): add the job
   `cargo build -p sc-treesitter --lib --no-default-features` and the same for
   `sc-ai`, plus `cargo tree -p sc-treesitter --no-default-features -e normal`
   asserting no `mimalloc`. Keep a-10's `cargo hack` matrix.
4. **REQ-SAG-0133 / ADR-SAG-0017 amendment.** Both `sc-ai` and
   `sc-treesitter` are released binaries. Per-channel coverage:

   | Channel | sc-ai | sc-treesitter |
   | --- | --- | --- |
   | GitHub Release archive | yes | yes |
   | crates.io (`cargo install`) | yes | yes |
   | Homebrew | yes | yes (same formula) |
   | Scoop | shim | in archive, no shim |
   | WinGet | alias | in package, no alias |

   The Scoop/WinGet single-shim limit is a sc-publish kit limitation. File
   bead `agt-scoop-multi-shim` and an upstream sc-publish issue, and record
   its URL in the ADR. `docs/development/sc-publish.md` documents the
   Windows invocation path. The consumer input is unchanged apart from any
   kit-supported field for extra binaries.
5. **ADR-SAG-0016 version text:** `"1.5.0"` → `=1.5.0` (exact, lockstep per
   ADR-SAG-0023).
6. **ADR-SAG-0005 narrowing (D13):** "runs the four consumer extractor kinds
   (`file`, `table-column`, `subjects`, `command`)"; the assembly entry point
   is named by ADR-SAG-0024.
7. **ADR-SAG-0007 / REQ-SAG-0505 retry numbers** (amendment text only;
   r-3 encodes them in `BusyPolicy`): statuses 429/502/529; Retry-After
   integer or decimal seconds clamped 0–30; missing → 1 s; malformed → 30 s;
   jitter ≤ 250 ms; 4 attempts; 90 s total budget from first send, where no
   new attempt starts after the budget and the in-flight attempt is bounded by
   the 120 s request timeout. Reviewer flag recorded in the amendment: the
   90 s budget is shorter than one request timeout, and three malformed-header
   sleeps alone use up the budget. Accepted for Phase A.1.
8. Flip ADR-SAG-0023 → Active and the r-4-owned amendment bullets → Active.
   Add the new ADR rows to the index table if it exists after a-10.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:NFR-SAG-0004 | Every `=` in any Cargo.toml is on the ADR-SAG-0023 lockstep list (`scripts`/test that greps manifests) |
| req:REQ-SAG-0134 | CI job present; `--no-default-features` lib builds green; no mimalloc in that tree |
| req:REQ-SAG-0133 | ADR-SAG-0017 table matches the consumer input and rendered Homebrew formula; bead + upstream issue recorded |
| adr:ADR-SAG-0023 | Status Active |
| adr:ADR-SAG-0017 | Amendment Active |
| adr:ADR-SAG-0016 | Version bullet Active |
| adr:ADR-SAG-0005 | Narrowing amendment Active |
| adr:ADR-SAG-0007 | Retry numbers recorded |
| boundary:release | Consumer preflight + publish dry-run green |

## Required validation

```bash
just lint
just test
cargo build -p sc-treesitter --lib --no-default-features
cargo build -p sc-ai --lib --no-default-features
test -z "$(cargo tree -p sc-treesitter --no-default-features -e normal | grep mimalloc)"
cargo hack check -p sc-treesitter --feature-powerset --no-dev-deps
bash .github/scripts/run-consumer-preflight.sh
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-secrets -p sc-jev -p sc-jev-backends -p sc-jev-check -p sc-treesitter -p sc-ai
```

## This sprint does not close

New-crate release entries (r-3, r-3b); clap on the `cli` feature (r-3b);
upstream fix for the Scoop multi-shim limit.
