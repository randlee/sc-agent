# sc-lint boundaries

Phase A manifests land in sprint **a-1**. Each subdirectory names a boundary
id referenced in `docs/plans/phase-a/plan.md`:

| Boundary id | Directory (owner package) | Crate |
| --- | --- | --- |
| `agent-types` | `sc-agent-types` | `sc-agent-types` |
| `agent-secrets` | `sc-agent-secrets` | `sc-agent-secrets` |
| `jev` | `sc-jev` | `sc-jev` |
| `jev-backends` | `sc-jev-backends` | `sc-jev-backends` |
| `jev-check` | `sc-jev-check` | `sc-jev-check` |
| `treesitter` | `sc-treesitter` | `sc-treesitter` |
| `jev-py` | `sc-jev-py` | `sc-jev-py` |
| `sc-ai-cli` | `sc-ai` | `sc-ai` |

Sprint **a-1** commits stub manifests next to this README. Each directory name
is the Cargo package name (`sc-lint` requires that). `boundary_id` and `name`
carry the Phase A boundary id. `boundaries/planning.toml` sets
`current_sprint` to `A.1`.
