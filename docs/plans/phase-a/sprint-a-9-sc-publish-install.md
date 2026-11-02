---
id: a-9
phase: a
status: planned
closure_type: integration
target_boundary: release
track: publish
wave: 4
branch: sprint/a-9-sc-publish-install
worktree: sprint/a-9-sc-publish-install
recommended_agent: sai
recommended_model: gpt-5.6-sol-medium
must_follow: [a-8]
parallel_safe: []
requirements:
  - REQ-SAG-0130
  - REQ-SAG-0131
adrs:
  - ADR-SAG-0018
owned_paths:
  - "release/sc-publish-pin.toml"
  - "release/sc-publish-consumer-input.json"
  - "docs/development/sc-publish.md"
  - "README.sc-publish.md"
  - ".gitignore"
---

# Sprint a-9 — sc-publish kit install

## Outcome

Install the shared **[sc-publish](https://github.com/randlee/sc-publish)** kit
(manifest-driven release for all SC repos). Pin an exact kit revision, author a
complete **`release/sc-publish-consumer-input.json`** for sc-agent (Rust crate
order, `sc-jev-py` Python distribution, GitHub + **crates.io** + **PyPI**
channels), and run `install.py` so rendered manifests and workflows match policy.

## Deliverables

1. **REQ-SAG-0130** — `release/sc-publish-pin.toml` + consumer JSON declaring:
   workspace crates (`sc-agent-types`, `sc-agent-secrets`, `sc-jev-backends`,
   `sc-jev`, `sc-ai`), maturin project `sc-jev-py`, and enabled channels.
2. **REQ-SAG-0131** — `docs/development/sc-publish.md` (install, pin bump,
   dry-run drift check); `.gitignore` entries for `.sc-publish-kit/`, venv cache.
3. **ADR-SAG-0018** — Do not hand-edit rendered `publish-artifacts.toml` /
   channel contracts; change consumer input + re-install.

## Acceptance criteria

```bash
test -f release/sc-publish-pin.toml
test -f release/sc-publish-consumer-input.json
test -f docs/development/sc-publish.md
# Pin-qualified install (see docs/development/sc-publish.md for exact recipe)
SC_PUBLISH_REVISION="$(sed -n 's/^revision = "\([0-9a-f]*\)"/\1/p' release/sc-publish-pin.toml)"
git clone --depth 1 "https://github.com/randlee/sc-publish" .sc-publish-kit
git -C .sc-publish-kit fetch origin "$SC_PUBLISH_REVISION" --depth 1
git -C .sc-publish-kit checkout "$SC_PUBLISH_REVISION"
publish_python="$(python3 .sc-publish-kit/plugins/sc-publish/.github/scripts/bootstrap_sc_compose.py --venv .sc-publish-venv)"
"$publish_python" .sc-publish-kit/plugins/sc-publish/install.py --dry-run --input release/sc-publish-consumer-input.json .
test $? -eq 0
test -f release/publish-artifacts.toml
```

Expected: dry-run exit 0 (no drift after install commit); rendered manifests list
sc-agent crates and `sc-jev-py`.

## This sprint does not close

Live publish to crates.io/PyPI (a-10); homebrew/winget unless added to consumer input.
