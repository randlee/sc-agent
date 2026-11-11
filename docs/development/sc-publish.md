# sc-publish release kit

sc-agent publishes through the shared
**[sc-publish](https://github.com/randlee/sc-publish)** kit (ADR-SAG-0017,
ADR-SAG-0018). The kit owns release workflows, scripts, agents, and rendered
manifests. This repository owns the pin, consumer input JSON, and the PR
dry-run gate.

After install, see [`README.sc-publish.md`](../../README.sc-publish.md),
[`docs/getting-started.md`](../getting-started.md), and
[`docs/consumer-preflight-and-docli.md`](../consumer-preflight-and-docli.md).

## Files

| File | Owner | Role |
| --- | --- | --- |
| `release/sc-publish-pin.toml` | sc-agent | Exact 40-character kit SHA |
| `release/sc-publish-consumer-input.json` | sc-agent | The only release manifest you edit |
| `release/publish-artifacts.toml` | kit (rendered) | Crates, binaries, targets, Python dists, channels |
| `release/publish-channel-contracts.toml` | kit (vendored) | Channel identity, secret names, registry endpoints |
| `release/publish-preflight.toml` | kit (rendered) | Consumer preflight checks |
| `.github/workflows/release*.yml`, `*-publish.yml`, `.github/scripts/`, `.github/actions/` | kit | Release pipeline |
| `.claude/agents/*-publisher.md`, `.cursor/agents/publisher.md`, `.claude/skills/publishing/` | kit | Publisher agents |
| `scripts/sc-publish-dry-run.sh` | sc-agent | Local and PR release dry-run |
| `.github/workflows/sc-publish-dry-run.yml` | sc-agent | Runs the dry-run on every PR |

Never hand-edit kit or rendered files (REQ-SAG-0131). Change consumer input or
the pin, then re-run install.

## Consumer input (current workspace)

- **crates.io** (publish order): `sc-agent-types`, `sc-decision-schema`,
  `sc-agent-config`, `sc-agent-secrets`, `sc-agent-log`, `sc-agent-http`,
  `sc-decision-systemone`, `sc-decision-local`, `sc-llm-provider`,
  `sc-decision-llm`, `sc-decision-runner`, `sc-treesitter`, `decidr`.
- **PyPI**: `sc-agent` and `sc-treesitter` maturin wheels (see
  `python_distributions` in consumer input).
- **Release binaries**: `decidr`, `sitr` (five-target matrix in consumer input).
- **Homebrew / Scoop / WinGet**: configured in consumer input (`Formula/decidr.rb`,
  `bucket/decidr.json`, `randlee.decidr`).

## Install

Clone the kit at the pinned SHA into `.sc-publish-kit/` (gitignored). Bootstrap
the renderer into `.sc-publish-venv/` (gitignored).

```bash
SC_PUBLISH_REVISION="$(sed -n 's/^revision = "\([0-9a-f]*\)"/\1/p' release/sc-publish-pin.toml)"
git clone --depth 1 "https://github.com/randlee/sc-publish" .sc-publish-kit
git -C .sc-publish-kit fetch origin "$SC_PUBLISH_REVISION" --depth 1
git -C .sc-publish-kit checkout "$SC_PUBLISH_REVISION"
publish_python="$(python3 .sc-publish-kit/plugins/sc-publish/.github/scripts/bootstrap_sc_compose.py --venv .sc-publish-venv)"
"$publish_python" .sc-publish-kit/plugins/sc-publish/install.py \
  --input release/sc-publish-consumer-input.json .
"$publish_python" .sc-publish-kit/plugins/sc-publish/install.py \
  --dry-run --input release/sc-publish-consumer-input.json .   # must exit 0
```

## Dry-run

```bash
bash scripts/sc-publish-dry-run.sh
SC_PUBLISH_ALLOW_DIRTY=1 bash scripts/sc-publish-dry-run.sh   # with uncommitted kit output
```
