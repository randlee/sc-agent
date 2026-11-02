# sc-publish (planned — sprint a-9)

Phase A closes with the shared **[sc-publish](https://github.com/randlee/sc-publish)**
kit (same as sc-runtime and sibling repos).

Until sprint **a-9** lands, this repo has kit-aligned workflow **stubs** only.
Do not hand-edit rendered files under `release/` once install has run.

## After a-9 (expected layout)

| File | Role |
| --- | --- |
| `release/sc-publish-pin.toml` | Exact upstream kit git SHA |
| `release/sc-publish-consumer-input.json` | Crates, `sc-jev-py`, channels (crates.io, PyPI, GitHub) |
| `release/publish-artifacts.toml` | Rendered by install |
| `release/publish-channel-contracts.toml` | Rendered by install |
| `release/publish-preflight.toml` | Rendered by install |

Install recipe and qualification steps will match
`docs/plans/phase-a/sprint-a-9-sc-publish-install.md` acceptance criteria.
