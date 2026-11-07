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

## Manifest conventions (Phase A phase-end)

- **`allowed_dependencies` lists every direct normal dependency** of the crate,
  workspace and third-party, as written in that crate's `Cargo.toml`
  (optional and target-specific dependencies included, dev-dependencies
  excluded). When a `Cargo.toml` gains or loses a normal dependency, update
  the manifest in the same change.
- **`forbidden_edges` are direct edges only.** Transitive reachability is not
  checked.
- **`[references]`, `[testing]` and `[enforcement]` are required by the
  sc-lint 0.8.0 schema.** `references.forbidden` is left empty because the
  validator accepts entries there but reports nothing for them. `[testing]`
  names the feature-gated test doubles. `enforcement.lint_rules` lists the
  rules below; sc-lint does not read that field.

## What `just lint` enforces

| Rule | Source | Checks |
| --- | --- | --- |
| `SCB-DEPENDENCY-001` | `allowed_dependencies` | A direct workspace-to-workspace edge that the manifest does not list |
| `SCB-DEPENDENCY-002` | `allowed_dependents` | A workspace crate depending on this one without permission |
| `SCB-DEPENDENCY-003` | `forbidden_edges` | A forbidden direct workspace-to-workspace edge that exists |
| `SCB-DEPENDENCY-004` | `allowed_dependents` | A permission with no matching edge (stale) |
| `SCB-MANIFEST-004` | `sc-lint.toml` `global_dependency_ownership` | A third-party crate (`ureq`, `pyo3`, `tree-sitter*`, `keyring`, `sc-observability`, `sc-observability-types`, `mimalloc`) declared or imported outside its owning crates |

sc-boundary 0.8.0 accepts third-party names in `allowed_dependencies` and
`forbidden_edges` but does not check them. The ownership records in
`sc-lint.toml` are what make the third-party forbidden edges mechanical; keep
them in step with the manifests. Enforcement is Cargo-edge only: no rule
restricts which code may call a given item (for example
`ResolvedApiKey::as_str`).

`sc-lint.toml` is marked "Managed by sc-lint". `sc-lint init --just`
regeneration would drop the hand-added `global_dependency_ownership` records
(and the `jev-python` test layer and the justfile recipes); re-apply them after
regenerating.
