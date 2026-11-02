# Repository Quality Policy

Repository-specific QA policy for **sc-agent**. Reusable agents and skills read
this file instead of embedding repo names, commands, or exceptions in prompts.

## Repository baseline

- Default comparison branch: `develop`
- Requirements index: `docs/requirements.md`
- Architecture index: `docs/architecture.md`
- Product design: `docs/design.md`
- Framework charter: `docs/framework-charter.md`
- Project plan: `docs/project-plan.md`
- Team protocol: `docs/team-protocol.md`
- Cross-platform testing: `docs/cross-platform-guidelines.md`
- Crate docs (when present): `docs/<crate>/requirements.md` and
  `docs/<crate>/architecture.md`
- Developer roster: `docs/development/developer-roster.md`

## ATM team (sc-ai)

- Team name: `sc-ai` (see `.atm.toml`)
- Lead identity: `ai-lead` — pass `lead=ai-lead` in orchestration template vars
- QA enforcer identity: `ai-qa` (quality-mgr role; prompt:
  `.claude/agents/quality-mgr.md`)
- Codex developers: `tai`, `lai`, `sai`

## Reviewer policy

- Initial implementation review: `req-qa`, `arch-qa`, `extensibility-qa`,
  `ruthless-boundary-qa`, `rust-qa-agent`, `rust-best-practices-agent`, and
  `rust-service-hardening-agent`
- Fix verification: `req-qa`, `arch-qa`, and `rust-qa-agent`; dispatch an
  additional reviewer only when rechecking a carry-forward finding it owns
- Plan review QA-1: `req-qa`, `arch-qa`, `extensibility-qa`,
  `ruthless-boundary-qa`, `rust-best-practices-agent`,
  `rust-service-hardening-agent`, and `ceremony-qa`
- Plan review QA-2 and later: `req-qa` and `arch-qa` scoped to dispatched
  findings; plan QA capped at 3 rounds (`plan_qa_cycle_limit`)
- `ceremony-finding-screen`: every sprint or plan QA round with findings,
  before the report is posted
- Phase-end review: initial implementation set plus `flaky-test-qa`
- `schema-reviewer`: only when governed interfaces in scope (see below)
- Every QA round requires an open PR and a posted findings report

## Validation policy

Until a Rust workspace lands on `develop`, sprint validation is **docs and agent
asset review only** (no `cargo` gate).

When `Cargo.toml` exists at the repository root:

- Sprint QA: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`,
  `cargo test --workspace`
- Phase-end artifact command: same as sprint unless a phase plan overrides it

## Release and publishing

- Manifest: `release/publish-artifacts.toml` (minimal until crates ship)
- Channel contracts: `release/publish-channel-contracts.toml`
- Helpers: `.github/scripts/release_artifacts.py`, `worker_result.py`,
  `bootstrap_sc_compose.py`
- Publishing workflows under `.github/workflows/` are present for kit sync;
  do not run release until the manifest declares real artifacts.

## Repository layout (reviewer expectations)

| Path | When required |
| --- | --- |
| `docs/requirements.md`, `docs/architecture.md` | Every review |
| `docs/design.md`, `docs/framework-charter.md` | Plan and architectural review |
| `docs/project-plan.md` | When a phase is active |
| `docs/plans/phase-<phase>/` | Plan and sprint reviews for that phase |
| `docs/<crate>/requirements.md`, `docs/<crate>/architecture.md` | Sprint owns or references that crate |
| `crates/<crate>/` or root `Cargo.toml` | Rust implementation sprints |
| `.claude/project/quality-policy.md` | Every QA round |
| `.claude/assets/sc-rust/` | Rust QA orchestration |

Absent paths marked “when present” or not yet required are **not** Blocking in
early bootstrap reviews.

## Governed interfaces

None declared yet. When added, list JSON Schema paths, OpenAPI specs, or MCP
tool manifests here for `schema-reviewer`.

## Plan naming

Lower case only.

| Thing | Form | Example |
| --- | --- | --- |
| Phase id | unused letter pair | `a` |
| Sprint id | `<phase>-<n>` | `a-1` |
| Plan directory | `docs/plans/phase-<phase>/` | `docs/plans/phase-a/` |
| Phase plan | `phase-<phase>-plan.md` | `phase-a-plan.md` |
| Sprint doc | `sprint-<phase>-<n>-<slug>.md` | `sprint-a-1-bootstrap.md` |

## Architectural rules

- Agent and skill paths are repo-root-relative; no host-specific paths in
  committed prompts.
- Orchestration dispatches to ATM identities declared in `.atm.toml`.

## Repository exceptions

None yet.
