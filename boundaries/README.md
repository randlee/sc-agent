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

## Dev-dependency and self edges

Recorded by sprint r-1's D0 spike (RBQA-F101, RBQA-F104), run with
sc-lint 0.8.0 (`sc-lint lint sc-boundary --json`) on a throwaway copy of the
tree at `34dfed0`. Rule ids observed:

| Case | Manifest edit | Findings |
| --- | --- | --- |
| 1. Dev-only edge between two workspace crates (`sc-agent-secrets` dev-depends on `sc-jev`, no normal edge) | none | `SCB-DEPENDENCY-001` ("directly dev-depends on `sc-jev` but `sc-jev` is not listed in ... `allowed_dependencies` or `allowed_dev_dependencies`"), `SCB-DEPENDENCY-002` (not in `sc-jev` `allowed_dependents`) |
| 1b. Same edge | `sc-jev` in `allowed_dev_dependencies` of `agent-secrets`, `sc-agent-secrets` in `allowed_dependents` of `jev` | pass |
| 2. Self edge `sc-agent-types = { path = ".", features = ["test-util"] }` (no `version` key) | none | `SCB-DEPENDENCY-001`, `SCB-DEPENDENCY-002`, `SCB-MANIFEST-002` ("path dependency version must match target crate version") |
| 2b. Same self edge, no `version` key | `sc-agent-types` in its own `allowed_dependencies` (or `allowed_dev_dependencies`) and `allowed_dependents` | `SCB-MANIFEST-002` (no manifest entry clears it) |
| 2c. Self edge with `version = "0.1.0"` | self entries as in 2b | pass, but `cargo package -p sc-agent-types` then fails ("no matching package named `sc-agent-types` found ... crates.io index"): a versioned self dev-dependency is uploaded and must resolve from the registry (CPR-M210) |
| 3. `allowed_dependents` entry with no edge behind it (`jev` allows `sc-treesitter`) | — | `SCB-DEPENDENCY-004` (stale permission) |
| 3b. `allowed_dependencies` entry with no edge behind it | — | none (outgoing allowlists are not checked for staleness) |
| Empty facade: `facade = ""` with full `[references]`, `[testing]`, `[enforcement]`, `[status]` blocks | — | rejected at load: `CLI.CONFIG_ERROR`, "defines an empty public.facade" (exit 3) |

**Outcome: dev edges counted, self edge flagged and self entry rejected.**
Dev-only workspace edges are counted (`SCB-DEPENDENCY-001`/`-002`; a
dependent permission with no live edge of any category is `-004`). A self
dev-dependency in the only publishable form (path-only, no `version` key) is
flagged by `SCB-MANIFEST-002`, and no boundary-manifest entry clears that
finding (only an `exclude_paths_by_rule` record in `sc-lint.toml` would,
which no manifest owns). So, from r-1 on:

- No crate gets a self dev-dependency. Each integration target that needs a
  `test-util` (or other non-default) hook is declared
  `[[test]] name = "<t>" required-features = ["test-util"]`, and every
  validation line for it passes `--features test-util`; a plain `cargo test`
  then skips the target instead of failing to compile. In `cargo test
  --workspace` the feature is usually on anyway, because other members'
  dev-dependencies enable it and Cargo unifies features across the selected
  packages. The `test_util_off` gate is unchanged: a command-line feature is
  not a dependency edge.
- r-1 applies this to `sc-agent-types`: targets `mock` (`test-util`) and
  `git` (`git`) carry `required-features`; `agent-types.toml` lists no self
  entry.
- Later sprint lines this branch changes: r-2 `sc-jev-check`, r-3
  `sc-agent-http`, r-2b `sc-jev` / `sc-jev-backends`, r-3b `sc-ai` use
  `[[test]] required-features` instead of the self dev-dependency their docs
  name, and their validation lines gain `--features test-util`. The
  "Correction" sentence of the ADR-SAG-0022 r-2 amendment
  (`docs/architecture.md`), which says `sc-jev-check`'s integration targets
  enable `test-util` through a self-referencing dev-dependency, is reworded
  by r-2 to "through `[[test]] required-features = ["test-util"]` and
  `--features test-util`".
- Cross-crate dev-only edges need manifest entries: the dependency in the
  depending crate's `allowed_dev_dependencies` (or `allowed_dependencies`)
  and the dependent in the target's `allowed_dependents`. r-3b's
  `workspace-tests` takes the "counted" branch (`sc-agent-types`,
  `sc-jev-check`, `sc-agent-compose` add `workspace-tests` to
  `allowed_dependents`).
- `facade = ""` is rejected, so r-3b's `workspace-tests` manifest uses
  `facade = "workspace_tests"` (plan.md "New boundary manifests").
