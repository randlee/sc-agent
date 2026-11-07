---
id: a-9
phase: a
status: complete
closure_type: integration
target_boundary: release
track: publish
wave: 5
branch: sprint/a-9-sc-publish-install
worktree: sprint/a-9-sc-publish-install
recommended_agent: sai
recommended_model: gpt-5.6-sol-medium
must_follow: [a-7, a-8]
parallel_safe: []
requirements:
  - REQ-SAG-0130
  - REQ-SAG-0131
  - REQ-SAG-0132
  - REQ-SAG-0803
  - REQ-SAG-0802
  - NFR-SAG-0002
adrs:
  - ADR-SAG-0011
  - ADR-SAG-0017
  - ADR-SAG-0018
  - ADR-SAG-0022
owned_paths:
  - "release/sc-publish-pin.toml"
  - "release/sc-publish-consumer-input.json"
  - "release/publish-artifacts.toml"
  - "release/publish-channel-contracts.toml"
  - "release/publish-preflight.toml"
  - "docs/development/sc-publish.md"
  - "README.sc-publish.md"
  - ".gitignore"
  - ".github/workflows/release-preflight.yml"
  - ".github/workflows/release.yml"
  - ".github/workflows/pypi-publish.yml"
  - "scripts/sc-publish-dry-run.sh"
  - ".github/workflows/sc-publish-dry-run.yml"
  - "crates/sc-jev-py/Cargo.toml"
  - "crates/sc-jev-py/pyproject.toml"
---

# Sprint a-9 — sc-publish kit, preflight, first-release dry-run

## Outcome

Install the shared **[sc-publish](https://github.com/randlee/sc-publish)** kit
(manifest-driven release for all SC repos). Pin an exact kit revision, author a
complete **`release/sc-publish-consumer-input.json`** for sc-agent (Rust crate
order, `sc-jev-py`, **`sc-treesitter`**, **`sc-ai` release binaries**, and channels
**crates.io**, **PyPI**, **homebrew**, **scoop**, **winget**), run `install.py`,
then prove the release dry-run (drift check, preflight, publish dry-run) in PR CI.

## Deliverables

1. **REQ-SAG-0130** — `release/sc-publish-pin.toml` + consumer JSON declaring:
   workspace crates (`sc-agent-types`, `sc-agent-secrets`, `sc-jev`,
   `sc-jev-backends`, `sc-jev-check`, **`sc-treesitter`**, `sc-ai`), maturin project `sc-jev-py`,
   **`release_binaries`** for `sc-ai` and **`sc-treesitter`** (platform matrix per kit), and channels
   **crates.io**, **pypi**, **homebrew**, **scoop**, **winget**.
2. **REQ-SAG-0131** — `docs/development/sc-publish.md` (install, pin bump,
   dry-run drift check); `.gitignore` entries for `.sc-publish-kit/`, venv cache.
3. **REQ-SAG-0132** — consumer-owned `sc-publish dry-run` workflow
   (`.github/workflows/sc-publish-dry-run.yml`) green on every PR. It runs
   `scripts/sc-publish-dry-run.sh`, the documented local recipe: `install.py
   --dry-run` drift check, manifest validation, version lockstep, consumer
   preflight (`run-consumer-preflight.sh`), `cargo publish --dry-run`, and
   `maturin build`. The kit's Release Preflight (`release-preflight.yml`) is
   `workflow_dispatch`-only and is run by the publisher at release time.
   Evidence: PR #17, GitHub Actions run
   [42339767937](https://github.com/randlee/sc-agent/actions/runs/42339767937),
   check `sc-publish dry-run / dry-run`, green.
4. **REQ-SAG-0803** — First-release checklist: `cargo publish --dry-run` order
   for workspace crates (including **`sc-treesitter`**); `maturin build` for `sc-jev-py`;
   tag/release gate documented in `docs/development/sc-publish.md`.
5. **ADR-SAG-0018** — Do not hand-edit rendered `publish-artifacts.toml` /
   channel contracts; change consumer input + re-install.
6. **ADR-SAG-0017** — Phase A close satisfies **first release surfaces** via sc-publish — not ad-hoc workflow edits.
7. **Wheel + pytest CI (REQ-SAG-0802 proof for a-8)** — the `python` job of the
   consumer-owned `.github/workflows/sc-agent-ci.yml`, step
   `maturin develop + pytest (just test-python)`. It installs maturin 1.16.0
   into a Python 3.11 venv, runs `maturin develop --features
   extension-module,test-util -m crates/sc-jev-py/Cargo.toml`, then
   `pytest tests/fixtures/jev-python -q` on ubuntu-24.04, macos-15 and
   windows-2022. The `sc-publish dry-run` wheel step above is the
   publish-surface build (`extension-module` only, no pytest). This item was
   missing when a-9 merged; it landed in the Phase A phase-end hardening
   (PR #18, REQ-QA-003). `install.py` never writes the workflow.

**Decisions recorded in this sprint.** The PR gate is consumer-owned
(`scripts/sc-publish-dry-run.sh` plus `.github/workflows/sc-publish-dry-run.yml`)
because the kit ships no PR-triggered preflight; `install.py` never writes
either path, so ADR-SAG-0018 still holds. `sc-jev-py` ships to PyPI only
(ADR-SAG-0011): its `Cargo.toml` sets `publish = false` and it is not in the
consumer `crates` list. Its `pyproject.toml` `[project].version` is static
because the kit's `sync-python-version` rejects dynamic versions. The maturin
feature list keeps `test-util` off in the published wheel (ADR-SAG-0022).
The workspace `jev_cross_impl` test target (`tests/jev_cross_impl/`) is hosted
by `sc-jev-py` because that crate is `publish = false`, so no crates.io package
carries a `[[test]]` path outside its own directory.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0130 | Consumer JSON lists facade + all publish crates/channels |
| req:REQ-SAG-0131 | Install doc + pin bump recipe |
| req:REQ-SAG-0132 | `sc-publish dry-run` workflow green on PR (run 42339767937 on PR #17); kit Release Preflight is dispatch-only, run by the publisher at release |
| req:REQ-SAG-0803 | Dry-run publish order documented and exercised (runs 42339767937, 42341299125, 42341873748 on PR #17) |
| evidence:REQ-SAG-0802 | Boundary evidence only; the `req:` root is owned by **a-8**. `sc-agent-ci` job `python` builds the `test-util` extension and runs `tests/fixtures/jev-python` pytest on three OSes |
| boundary:release | Kit dry-run renders five channels |

## Required validation

```bash
set -o pipefail
just lint
just test
test -f release/sc-publish-pin.toml
test -f release/sc-publish-consumer-input.json
test -f docs/development/sc-publish.md
test -f release/publish-artifacts.toml
# Kit clone at the pin, install.py --dry-run, manifest + lockstep, consumer
# preflight, cargo publish --dry-run --locked (manifest order), maturin build.
SC_AGENT_WHEEL_DIR="$(mktemp -d)/sc-agent-wheels" bash scripts/sc-publish-dry-run.sh
```

Expected: dry-run exit 0; preflight exit 0; rendered manifests and workflows
include all five channels; publish dry-run and wheel build succeed on CI runner.

## This sprint does not close

Production publish of v0.x (owner-triggered tag after plan approval).
