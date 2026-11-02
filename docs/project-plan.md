# sc-agent project plan

**Status:** Phase A planned (awaiting approval)  
**Default branch:** `develop`

## Phases

| Phase | Status | Integration branch | Plan |
| --- | --- | --- | --- |
| Bootstrap | Complete | `develop` | Agent teams, plan-anchor, beads, policy |
| **A — sc-jev + sc-ai** | **Planned** | `integrate/phase-a` | [`docs/plans/phase-a/plan.md`](plans/phase-a/plan.md) |
| B (draft) | Not planned | — | Remaining Jev API (REQ-SAG-0110); check features 4–7; **API-direct** Claude + Codex via `sc-ai`. **Not** claude-code/codex-cli/cursor-cli wrappers |

## Lead

| Phase | Lead (ATM identity) |
| --- | --- |
| Bootstrap / A | `ai-lead` |

## Phase A sprints (dependency order)

1. a-1 workspace, errors, cross-impl fixture  
2. a-2 ∥ a-3 secrets ∥ check build  
3. a-4 ∥ a-5 Jev ∥ LLM backends  
4. a-6 sessions + orchestration  
5. a-7 ∥ a-8 CLI ∥ sc-jev-py wheel  
6. a-9 **sc-publish** kit install  
7. a-10 release preflight + publish dry-run  

**First release:** crates.io + PyPI + `sc-ai` via [sc-publish](https://github.com/randlee/sc-publish).

Sprint docs: `docs/plans/phase-a/sprint-a-*.md`.

## Approval gate

**No Rust implementation PRs** until Rand approves `plan.md` and sprint docs.

## After Phase A

- Migrate orchestration / plan-anchor call sites to the new client (separate repos).
- API-direct Claude and Codex agent crates (Phase B).
- Subprocess wrappers for claude-code, codex-cli, cursor-cli (never).
- Optional: `decider-4b` local backend crate.
