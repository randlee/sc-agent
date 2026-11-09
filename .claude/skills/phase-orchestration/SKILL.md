---
name: phase-orchestration
version: 0.2.0
description: Orchestrate multi-sprint phase execution as team-lead. Manages sprint waves, scrum-master lifecycle, PR merges, Codex design reviews, and integration branch strategy. This skill is for the TEAM-LEAD only, not for scrum-masters.
depends_on:
  scrum-master: 0.x
  rust-developer: 0.x
  rust-qa-agent: 0.x
  req-qa: 0.x
---

# Phase Orchestration

This skill defines how the team-lead orchestrates a development phase consisting of multiple sprints with dependency-aware parallelism.

**Audience**: Team-lead only. Scrum-masters have their own process defined in `.claude/agents/scrum-master.md`.

## Prerequisites

Before starting a phase:
1. Phase plan exists under `docs/plans/phase-<phase>/` (see
   `.claude/project/quality-policy.md` **Plan Naming**) with sprint specs
2. Integration branch `integrate/phase-<phase>` exists and is up to date with
   `develop`
3. The team named by `$ATM_TEAM` exists — do NOT recreate between phases
4. The persistent Codex reviewer is running and reachable via ATM CLI. Below it is called `<codex-reviewer>`; the repo's `docs/team-protocol.md` and developer roster name it.

## Phase Execution Loop

### 1. Build Sprint Dependency Graph

Read the phase plan and identify:
- Sprint dependencies (which sprints block others)
- Parallel waves (groups of sprints that can run concurrently)
- Merge order within each wave (to minimize conflicts on shared files)

### 2. Execute Sprints

For each sprint (respecting dependency order):

#### a. Spawn a Fresh Scrum-Master

Each sprint gets a **fresh** scrum-master — do NOT reuse scrum-masters across sprints.

```json
{
  "subagent_type": "scrum-master",
  "name": "sm-{phase}-{sprint}",
  "team_name": "<team-name>",
  "model": "sonnet",
  "prompt": "<sprint prompt — see template below>"
}
```

**Critical rules:**
- `subagent_type` MUST be `"scrum-master"` — it has built-in dev-QA loop orchestration
- `name` parameter IS required — scrum-masters are full tmux teammates that CAN spawn background sub-agents
- `team_name` IS required — they need team membership for SendMessage
- The scrum-master is a **COORDINATOR ONLY** — it spawns `rust-developer` and `rust-qa-agent` as background agents
- The scrum-master MUST NOT write code, run tests, or implement fixes itself
- If a scrum-master is found doing dev work, it is a bug in the orchestration

#### b. Sprint Prompt Template

```
You are the scrum-master for Phase {P}, Sprint {P}.{S}: {Title}.

PHASE PLAN: Read docs/plans/phase-{P}/phase-{P}-plan.md for full context.
SPRINT DOC: docs/plans/phase-{P}/sprint-{P}-{n}-{slug}.md (authoritative).
REQUIREMENTS: Read docs/requirements.md and crate docs as listed in the sprint.

WORKTREE:
- Create worktree via sc-git-worktree skill from integrate/phase-{P}
- Work in: ../<repo>-worktrees/sprint/{P}-{n}-{slug}
- Branch: sprint/{P}-{n}-{slug} (see quality-policy; do not use feature/)

PR targets: integrate/phase-{P}

REMINDER: You are a COORDINATOR. You spawn rust-developer and rust-qa-agent as
background agents. You do NOT write code, run cargo test, or implement fixes yourself.
Follow your standard dev-QA loop process (defined in your agent prompt).
When complete, send message to team-lead with PR number and summary.
```

#### c. Monitor Progress

- Scrum-masters report via SendMessage when done
- If a scrum-master reports subagent spawn failure, investigate and advise — do NOT tell it to do dev work itself
- If a scrum-master escalates, spawn a rust-architect (opus) for analysis and send findings back to scrum-master

### 3. Post-Sprint: CI Gate + Merge

After each scrum-master reports completion:

1. **Verify QA passed** — scrum-master should confirm QA agent gave PASS verdict
2. **Wait for CI** — poll PR checks until green (use delay-poll agent)
3. **Merge PR** to `integrate/phase-{N}` in dependency order
4. **Update integration branch** — pull latest into worktree
5. **Mark task completed** — TaskUpdate status to completed

### 4. Post-Sprint: Codex Critical Design Review

**After EVERY sprint PR is merged to `integrate/phase-{N}`**, request `<codex-reviewer>` review:

1. Send `<codex-reviewer>` the diff via ATM CLI:
   ```
   atm send <codex-reviewer> "Sprint {P}.{S} merged (PR #{N}). Critical design review requested. Review: gh pr diff {N} --repo <owner>/<repo>. Focus: correctness bugs, architectural violations, missing edge cases."
   ```
2. Start the next eligible sprint immediately (dependency permitting) — do NOT wait for `<codex-reviewer>` review before continuing development
3. Run `<codex-reviewer>` review in parallel (use delay agent, nudge via tmux if no reply in 2 min)
4. Track `<codex-reviewer>` findings:
   - **No issues**: Continue to next sprint
   - **Issues found**: Create/update a **parallel `<codex-reviewer>` fix track** in a separate worktree (`fix/{P}-arch-review`) to address findings while later sprint waves continue
5. `<codex-reviewer>` is authorized to implement fixes directly in the fix worktree for review findings
6. Every `<codex-reviewer>` fix PR MUST be validated by QA agents (`rust-qa-agent` and `req-qa`) before merge
7. Do NOT block ongoing sprint execution unless `<codex-reviewer>` marks findings as critical/blocking

### 5. Codex Review Fix Sprint (if needed)

If `<codex-reviewer>` found issues across sprints:
1. Create a new worktree branched from `integrate/phase-{N}` (after all sprint PRs merged)
2. `<codex-reviewer>` may execute fixes directly OR team-lead may delegate to a fresh scrum-master
3. Regardless of who implements fixes, run QA validation (`rust-qa-agent` + `req-qa`) before merge
4. Follow normal CI loop and merge fix PR to integration branch
5. Request `<codex-reviewer>` re-review of fixes if delegated implementation was used

### 6. Wave Transitions (for parallel sprints)

Before starting the next wave:
1. All prerequisite sprints from previous wave must be merged
2. Integration branch must be updated (`git pull` in worktree)
3. Any `<codex-reviewer>` critical/blocking findings must be addressed first
4. New scrum-masters get fresh worktrees branched from updated `integrate/phase-{N}`

### 7. Phase Completion

After all sprints (including fix sprint if needed) merge to `integrate/phase-{N}`:
1. Version bump (separate commit on integration branch)
2. Create PR: `integrate/phase-{N} → develop`
3. Wait for CI green
4. Merge after user approval
5. Shutdown all remaining scrum-master panes
6. Do NOT clean up worktrees until user reviews them

## Scrum-Master Lifecycle

- **Fresh per sprint** — each sprint gets a new scrum-master instance
- **Named tmux teammate** — spawned with `name` parameter for full CLI process
- **Can spawn sub-agents** — background rust-developer and rust-qa-agent (no `name` param)
- **Shutdown after sprint** — send shutdown_request after PR merges and CI passes
- **NEVER does dev work** — if you see a scrum-master writing code, the prompt is wrong

## Team Lifecycle

- **Team persists across phases** — NEVER use TeamDelete on persistent teams
- **Scrum-masters are ephemeral** — shutdown after their sprint completes
- **`<codex-reviewer>` is persistent** — communicates exclusively via ATM CLI (not Claude Code SendMessage)
- Between sprints: team stays alive, only scrum-master panes come and go

## ATM CLI Communication (Codex reviewer)

`<codex-reviewer>` is a Codex agent that does NOT receive Claude Code team messages. Use ATM CLI only:

```bash
atm send <codex-reviewer> "message"     # Send
atm read                         # Check replies
atm inbox                        # Summary
```

Nudge via tmux if no reply:
```bash
tmux send-keys -t <pane-id> -l "You have unread ATM messages. Run: atm read --team $ATM_TEAM" && sleep 0.5 && tmux send-keys -t <pane-id> Enter
```

## Task Tracking

Create one task per sprint at phase start:
- Set dependencies via addBlockedBy
- Assign owner when scrum-master starts
- Mark completed when PR merges

## Anti-Patterns

- Do NOT use `rust-developer` as scrum-master subagent_type — use `scrum-master`
- Do NOT tell scrum-masters to "do the work yourself" — they are coordinators
- Do NOT do dev or QA work as team-lead — delegate to scrum-masters
- Do NOT skip post-merge `<codex-reviewer>` sprint review — every merged sprint requires it
- Do NOT merge `<codex-reviewer>` fix PRs without QA validation from both QA agents
- Do NOT merge without QA pass + CI green
- Do NOT delete the team between sprints or phases
- Do NOT clean up worktrees without user approval
- Do NOT reuse scrum-masters across sprints — each sprint gets a fresh instance
