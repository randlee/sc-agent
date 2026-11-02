---
id: a-9
phase: a
status: planned
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
adrs:
  - ADR-SAG-0017
  - ADR-SAG-0018
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
---

# Sprint a-9 — sc-publish kit, preflight, first-release dry-run

## Outcome

Install the shared **[sc-publish](https://github.com/randlee/sc-publish)** kit
(manifest-driven release for all SC repos). Pin an exact kit revision, author a
complete **`release/sc-publish-consumer-input.json`** for sc-agent (Rust crate
order, `sc-jev-py`, **`sc-treesitter`**, **`sc-ai` release binaries**, and channels
**crates.io**, **PyPI**, **homebrew**, **scoop**, **winget**), run `install.py`,
then prove **release preflight** and publish dry-run in CI.

## Deliverables

1. **REQ-SAG-0130** — `release/sc-publish-pin.toml` + consumer JSON declaring:
   workspace crates (`sc-agent-types`, `sc-agent-secrets`, `sc-jev`,
   `sc-jev-backends`, `sc-jev-check`, **`sc-treesitter`**, `sc-ai`), maturin project `sc-jev-py`,
   **`release_binaries`** for `sc-ai` and **`sc-treesitter`** (platform matrix per kit), and channels
   **crates.io**, **pypi**, **homebrew**, **scoop**, **winget**.
2. **REQ-SAG-0131** — `docs/development/sc-publish.md` (install, pin bump,
   dry-run drift check); `.gitignore` entries for `.sc-publish-kit/`, venv cache.
3. **REQ-SAG-0132** — `release-preflight` workflow green on PR; documented
   local recipe matches CI (`run-consumer-preflight.sh` or kit equivalent).
4. **REQ-SAG-0803** — First-release checklist: `cargo publish --dry-run` order
   for workspace crates (including **`sc-treesitter`**); `maturin build` for `sc-jev-py`;
   tag/release gate documented in `docs/development/sc-publish.md`.
5. **ADR-SAG-0018** — Do not hand-edit rendered `publish-artifacts.toml` /
   channel contracts; change consumer input + re-install.
6. **ADR-SAG-0017** — Phase A close satisfies **first release surfaces** via sc-publish — not ad-hoc workflow edits.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0130 | Consumer JSON lists facade + all publish crates/channels |
| req:REQ-SAG-0131 | Install doc + pin bump recipe |
| req:REQ-SAG-0132 | Release preflight workflow green |
| req:REQ-SAG-0803 | Dry-run publish order documented and exercised |
| boundary:release | Kit dry-run renders five channels |

## Required validation

```bash
set -o pipefail
just lint
just test
test -f release/sc-publish-pin.toml
test -f release/sc-publish-consumer-input.json
test -f docs/development/sc-publish.md
SC_PUBLISH_REVISION="$(sed -n 's/^revision = "\([0-9a-f]*\)"/\1/p' release/sc-publish-pin.toml)"
git clone --depth 1 "https://github.com/randlee/sc-publish" .sc-publish-kit
git -C .sc-publish-kit fetch origin "$SC_PUBLISH_REVISION" --depth 1
git -C .sc-publish-kit checkout "$SC_PUBLISH_REVISION"
publish_python="$(python3 .sc-publish-kit/plugins/sc-publish/.github/scripts/bootstrap_sc_compose.py --venv .sc-publish-venv)"
"$publish_python" .sc-publish-kit/plugins/sc-publish/install.py --dry-run --input release/sc-publish-consumer-input.json .
test $? -eq 0
test -f release/publish-artifacts.toml
bash .github/scripts/run-consumer-preflight.sh
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-secrets -p sc-jev -p sc-jev-backends -p sc-jev-check -p sc-treesitter -p sc-ai
maturin build -m crates/sc-jev-py/Cargo.toml -o /tmp/sc-agent-wheels
test -n "$(ls /tmp/sc-agent-wheels/*.whl 2>/dev/null | head -1)"
```

Expected: dry-run exit 0; preflight exit 0; rendered manifests and workflows
include all five channels; publish dry-run and wheel build succeed on CI runner.

## This sprint does not close

Production publish of v0.x (owner-triggered tag after plan approval).
