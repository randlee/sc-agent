---
id: a-12
phase: a
status: planned
closure_type: integration
target_boundary: release
track: publish
wave: 8
branch: sprint/a-12-sc-publish-dry-run
must_follow: [a-11]
parallel_safe: []
requirements:
  - REQ-SAG-0803
adrs:
  - ADR-SAG-0017
owned_paths:
  - "scripts/sc-publish-dry-run.sh"
  - ".github/workflows/sc-publish-dry-run.yml"
---

# Sprint a-12 — publish dry-run in CI

## Outcome

Close REQ-SAG-0803: `cargo publish --dry-run` in manifest order, both maturin wheels, and optional full preflight rehearsal on PR CI.

## Deliverables

- Green `sc-publish-dry-run` workflow on integration branch.
- Documented local recipe matching CI in `docs/development/sc-publish.md`.

## This sprint does not close

Production `cargo publish`, PyPI upload, or Homebrew/Scoop/WinGet pushes (owner-triggered tag after plan approval).
