# Repository Quality Policy

Repository-specific QA policy for **sc-agent**. Reusable agents and skills read
this file instead of embedding repo names, commands, or exceptions in prompts.

(Structure follows Synaptic Canvas QA repos; **atm-core** does not ship a
`.claude/project/quality-policy.md` — this file is sc-agent's source of truth.)

## Repository baseline

**Raptor records replace the index documents.** This repository keeps its
requirements, non-functional requirements and architecture decisions as Raptor
records (ADR-SAG-0003): repository-wide records under `docs/requirements/`,
`docs/nfr/` and `docs/adr/`, and crate records under
`docs/<crate>/requirements/`. The phase plan lives under
`docs/plans/phase-<id>/`. Wherever this policy or a reviewer asks for
`docs/requirements.md`, `docs/architecture.md`, `docs/design.md`,
`docs/project-plan.md`, `docs/framework-charter.md` or
`docs/<crate>/requirements.md` / `docs/<crate>/architecture.md`, reviewers read
those record folders and the phase plan instead; the absence of the index files
is not a finding.

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
- Lead session skill: `.claude/skills/team-lead/SKILL.md`

## ATM team (sc-ai)

- Team name: `sc-ai` (see `.atm.toml`, `ATM_TEAM=sc-ai`)
- **Lead identity:** `ai-lead` — orchestration templates use `--var lead=ai-lead`
  (or `"lead": "ai-lead"` in vars JSON)
- **QA enforcer identity:** `ai-qa` — dispatches use member `ai-qa`; prompt file
  remains `.claude/agents/quality-mgr.md`
- **Developers:** `tai`, `lai`, `sai` (Codex)
- Session init: lead runs `/team-lead` at fresh session start

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
  findings; dispatch another reviewer only when rechecking a carry-forward
  finding it owns. Plan QA is capped at **3 rounds** (`plan_qa_cycle_limit`);
  a round that leaves only minor findings reports
  `PASS — minor fixes required, no re-QA`
- `ceremony-finding-screen`: every sprint or plan QA round that has findings,
  before the report is posted
- Phase-end review: initial implementation set plus `flaky-test-qa`
- Run `flaky-test-qa` earlier when tests changed or instability is suspected
- `schema-reviewer`: only when Governed Interfaces below names a relevant
  interface; otherwise `SKIPPED`
- Every QA round requires an open PR; the rendered report is posted every round

## Validation policy

Until a Rust workspace exists on `develop`, sprint validation is **docs and agent
asset review only** (no `cargo` gate).

When root `Cargo.toml` exists:

- Sprint QA: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`,
  `cargo test --workspace`
- Phase-end artifact command: `cargo fmt --check`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo test --workspace --all-features` unless a phase plan overrides it

## Release and publishing

- Manifest: `release/publish-artifacts.toml` (minimal until crates/channels ship)
- Channel contracts: `release/publish-channel-contracts.toml`
- Helpers: `.github/scripts/release_artifacts.py`, `worker_result.py`,
  `bootstrap_sc_compose.py`
- Workflows under `.github/workflows/` are kit-aligned (crates.io, PyPI, Homebrew,
  Scoop, WinGet); do not publish until sc-publish consumer input is installed and
  Release Preflight passes

## Repository layout (reviewer expectations)

Reviewers read this table before treating missing paths as Blocking.

| Path | When required |
| --- | --- |
| `docs/requirements/`, `docs/nfr/`, `docs/adr/` (Raptor records; replace `docs/requirements.md`, `docs/architecture.md`, `docs/design.md`, `docs/framework-charter.md`) | Every review |
| `docs/plans/phase-<phase>/plan.md` (replaces `docs/project-plan.md`) | When a phase is active |
| `docs/plans/phase-<phase>/` | Plan and sprint reviews for that phase |
| `docs/<crate>/requirements/` (Raptor records; replace `docs/<crate>/requirements.md`, `docs/<crate>/architecture.md`) | Sprint owns or references that crate and the folder exists |
| `crates/<crate>/` or root `Cargo.toml` | Rust implementation sprints |
| `.claude/project/quality-policy.md` | Every QA round |
| `.claude/assets/sc-rust/` | Rust QA orchestration |
| `.claude/skills/triaging-findings/` | Pre-dispatch finding correlation |
| `.claude/skills/closing-triage/scripts/query_open_findings.py` | Fix rounds when present |

Absent paths marked not yet required are **not** Blocking in bootstrap reviews.

## Governed interfaces

None declared. Until this section names an interface, its compatibility rule,
evidence paths, and approval authority, `schema-reviewer` returns `SKIPPED`.

## Plan naming

Lower case only.

| Thing | Form | Example |
| --- | --- | --- |
| Phase id | next unused letter(s) | `a` |
| Sprint id | `<phase>-<n>` | `a-1` |
| Plan directory | `docs/plans/phase-<phase>/` | `docs/plans/phase-a/` |
| Phase plan | `plan.md` under `docs/plans/phase-<phase>/` (alias `phase-<phase>-plan.md` also allowed) | `docs/plans/phase-a/plan.md` |
| Sprint doc | `sprint-<phase>-<n>-<slug>.md` | `sprint-a-1-bootstrap.md` |
| Plan branch | `plan/phase-<phase>`, PR to `develop` | `plan/phase-a` |
| Phase branch | `integrate/phase-<phase>`, from `develop` | `integrate/phase-a` |
| Sprint branch | `sprint/<phase>-<n>-<slug>` | `sprint/a-1-bootstrap` |
| Fix layer | `fix/<phase>-<n>-<slug>` | `fix/a-1-qa1` |

Phase plans live only under `docs/plans/phase-<phase>/`.

## Architectural rules

- Agent and skill paths are repo-root-relative; no host-specific paths in
  committed prompts.
- Orchestration dispatches to ATM identities in `.atm.toml` and this policy.

## Repository exceptions

None yet.
