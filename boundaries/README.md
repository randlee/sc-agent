# sc-lint boundaries

Phase A manifests land in sprint **a-1**. Each subdirectory names a boundary
id referenced in `docs/plans/phase-a/plan.md`:

| Boundary id | Crates |
| --- | --- |
| `agent-types` | `sc-agent-types` |
| `agent-secrets` | `sc-agent-secrets` |
| `jev` | `sc-jev` |
| `jev-backends` | `sc-jev-backends` |
| `jev-check` | `sc-jev-check` |
| `treesitter` | `sc-treesitter` |
| `jev-py` | `sc-jev-py` |
| `sc-ai-cli` | `sc-ai` |

Manifest files (`boundary.toml` or sc-lint convention used by sibling repos)
are **not** committed until implementation PR **a-1**. Sprint **a-1** owns
`boundaries/**` stub manifests (not only this README).
