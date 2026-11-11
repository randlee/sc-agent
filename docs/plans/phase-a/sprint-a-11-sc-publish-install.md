---
id: a-11
phase: a
status: in_progress
closure_type: integration
target_boundary: release
track: publish
wave: 7
branch: sprint/a-11-sc-publish-install
worktree: sprint-a-9-sc-publish-install
must_follow: [a-10]
parallel_safe: []
requirements:
  - REQ-SAG-0130
  - REQ-SAG-0131
  - REQ-SAG-0132
  - REQ-SAG-0803
adrs:
  - ADR-SAG-0004
  - ADR-SAG-0017
  - ADR-SAG-0018
owned_paths:
  - "release/sc-publish-pin.toml"
  - "release/sc-publish-consumer-input.json"
  - "release/publish-artifacts.toml"
  - "release/publish-channel-contracts.toml"
  - "release/publish-preflight.toml"
  - "release/docli-pin.toml"
  - "docs/development/sc-publish.md"
  - "README.sc-publish.md"
  - "scripts/generate-cli-docs.sh"
  - "site/cli/"
  - "fixtures/cli/"
  - "Cargo.toml"
  - ".gitignore"
---

# Sprint a-11 — sc-publish kit and five channels

## Outcome

Install [sc-publish](https://github.com/randlee/sc-publish) at pin `44cebf2`, declare consumer input for **decidr**, **sitr**, thirteen crates.io crates, PyPI **sc-agent** / **sc-treesitter**, and five channels; enable **docli** CLI reference for both binaries; prove `install.py --dry-run` and manifest validation.

## Deliverables

1. REQ-SAG-0130 consumer JSON + literal workspace members.
2. REQ-SAG-0131 install doc and pin bump recipe.
3. REQ-SAG-0132 preflight workflow + docli drift check.
4. REQ-SAG-0803 groundwork (dry-run script present; full exercise closes in a-12).
5. ADR-SAG-0017 and ADR-SAG-0018.

## Validation

```bash
.sc-publish-venv/bin/python .sc-publish-kit/plugins/sc-publish/install.py \
  --dry-run --input release/sc-publish-consumer-input.json .
python3 .github/scripts/release_artifacts.py validate-manifest \
  --manifest release/publish-artifacts.toml --workspace-toml Cargo.toml
bash .github/scripts/release-preflight-docli.sh
cargo tree -p decidr --depth 1
cargo tree -p sc-treesitter --depth 1
```

Layering (ADR-SAG-0004): `version.workspace` / member list changes only; `cargo tree` depth-1 edges unchanged from a-10 integration.
