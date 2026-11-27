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

Never hand-edit kit or rendered files. Change consumer input or
the pin, then re-run install.

## Consumer input (current workspace)

- **crates.io**: publish `sc-agent`, `sc-treesitter`, `sitr`, then `decidr`, in the dependency order declared by consumer input. `sc-agent-py` stays unpublished on crates.io and ships as the wheel.
- **Current release scope**: those crates, the `sc-decidr` wheel and `decidr`/`sitr` binaries through PyPI, GitHub Releases, Homebrew, Scoop and WinGet.
- **PyPI**: one `sc-decidr` wheel containing decisions and sitr.
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

## Bump the kit pin

Set `release/sc-publish-pin.toml` to the qualified upstream 40-character commit SHA. Fetch and check out that same revision in `.sc-publish-kit/`, edit `release/sc-publish-consumer-input.json` for this workspace, then render the manifests:

```bash
python3 .sc-publish-kit/plugins/sc-publish/install.py --input release/sc-publish-consumer-input.json .
python3 .sc-publish-kit/plugins/sc-publish/install.py --dry-run --input release/sc-publish-consumer-input.json .
```

Review the pin, consumer input and generated diff together. The kit derives its crates.io root channel from the crate entries; `channels` in consumer input lists only post-release channels. Run `python3 .github/scripts/release_artifacts.py validate-manifest --manifest release/publish-artifacts.toml --workspace-toml Cargo.toml`, then `bash scripts/sc-publish-dry-run.sh` before release. Commit the qualified pin and generated changes in the same PR.
