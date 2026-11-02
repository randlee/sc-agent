---
id: a-10
phase: a
status: planned
closure_type: integration
target_boundary: release
track: publish
wave: 4
branch: sprint/a-10-release-preflight
worktree: sprint/a-10-release-preflight
recommended_agent: sai
recommended_model: gpt-5.6-sol-medium
must_follow: [a-9]
parallel_safe: []
requirements:
  - REQ-SAG-0132
  - REQ-SAG-0803
adrs:
  - ADR-SAG-0017
owned_paths:
  - ".github/workflows/release-preflight.yml"
  - ".github/workflows/release.yml"
  - ".github/workflows/pypi-publish.yml"
  - "release/publish-preflight.toml"
---

# Sprint a-10 — release preflight and publish dry-run

## Outcome

**Release Preflight** and publish workflows (from sc-publish kit) pass on
`integrate/phase-a`. First release train is **provable** via dry-run for
**crates.io**, **PyPI**, **homebrew**, **scoop**, and **winget** (plus GitHub
release assets) without production credentials in CI where the kit allows.

## Deliverables

1. **REQ-SAG-0132** — `release-preflight` workflow green on PR; documented
   local recipe matches CI (`run-consumer-preflight.sh` or kit equivalent).
2. **REQ-SAG-0803** — First-release checklist: `cargo publish --dry-run` order
   for workspace crates; `maturin build` for `sc-jev-py`; tag/release gate
   documented in `docs/development/sc-publish.md`.

## Acceptance criteria (authoritative)

| Root | Criterion |
| --- | --- |
| req:REQ-SAG-0132 | Release preflight workflow green |
| req:REQ-SAG-0803 | Dry-run publish order documented and exercised |

## Required validation

```bash
set -o pipefail
just lint
just test
bash .github/scripts/run-consumer-preflight.sh
cargo publish --dry-run --locked -p sc-agent-types -p sc-agent-secrets -p sc-jev -p sc-jev-backends -p sc-jev-check -p sc-ai
maturin build -m crates/sc-jev-py/Cargo.toml -o /tmp/sc-agent-wheels
test -n "$(ls /tmp/sc-agent-wheels/*.whl 2>/dev/null | head -1)"
# Five-channel proof: consumer preflight exercises homebrew/scoop/winget render + dry-run per kit contract
```

Expected: preflight exit 0; dry-run publish and wheel build succeed on CI runner
(equivalent commands in workflow).

## Phase A close

With a-10 merged, Phase A satisfies **first release surfaces** (ADR-SAG-0017)
via sc-publish — not ad-hoc workflow edits.

## This sprint does not close

Production publish of v0.x (owner-triggered tag after plan approval).
